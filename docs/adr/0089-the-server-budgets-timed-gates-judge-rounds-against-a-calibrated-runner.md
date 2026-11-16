# ADR-0089: The server budgets' timed gates judge rounds, against a calibration of the runner

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §7 *Server-budget timing gates read runner noise on shared CI*. Amends ADR-0080 (cold start
  and the list: how many, in what order) and ADR-0082 (patch latency: one stream). The budgets themselves
  (`tools/budgets.ts` `SERVER_BUDGETS`, ADR-0011) are unchanged.

## Context

`spikes/server-budgets` gated three timed budgets on one measure each: cold start the median of 7 restarts, the list
the median of 30 reads, patch latency the p99 of one 10 s stream (800 samples). On GitHub's shared runners that failed
on the runner, not on wisp. Every Linux job PR CI logged for the code since ADR-0084 (`/tmp/ci-*.log`, runs df56a408,
9dcd…, 4612442b; two jobs a run, the same build in both):

| job | cold start median (restarts) | list median (min · p95 · max) | patch p99 (p50 · max) |
|---|---|---|---|
| df56 full | 116.3 (105.1–134.1) | 3.86 (2.69 · 6.66 · 6.80) | 104.7 (87.2 · 105.6) |
| df56 quick | 105.4 (78.2–116.3) | 4.47 (2.79 · 10.11 · 22.73) | 103.7 (92.5 · 104.4) |
| 9dcd full | 111.2 (99.7–127.0) | 4.39 (2.91 · 11.75 · 28.29) | 103.6 (91.8 · 104.3) |
| 9dcd quick | 103.6 (81.1–**226.2**) | 2.64 (1.55 · 3.95 · 6.23) | 104.2 (87.3 · 104.9) |
| 4612 full | 99.0 (85.5–**424.5**) | 4.69 (2.45 · 9.94 · 11.30) | **170.4** ❌ (90.6 · 172.8) |
| 4612 quick | 121.1 (104.8–146.9) | **7.87** ❌ (4.61 · 67.41 · 102.47) | 109.9 (87.0 · 113.0) |

- **The list's miss was a slow spell**: in 4612's quick job the 30 reads (about 4 s) had a p90 of 67 ms against a
  usual 3–5; the same build read 4.69 in the other job. One block of consecutive reads is one sample of the runner.
- **The patch miss was one stalled commit window**: p99 170.4 with max 172.8 and p50 90.6. 10 tasks × 2 stamps a
  second put ~2 stamps in each 100 ms commit window, to 4 clients: 8 samples, exactly the 1 % of 800 a p99 lets
  through. One ~70 ms stall anywhere in 10 s decides the gate.
- **Cold start's restarts have outliers** (424.5, 226.2) that the median of 7 absorbed; 7 back-to-back restarts
  (about 1 s) are still one window of the runner.
- Rerunning the job until it passes is a retry: AGENTS.md calls a test that fails only some of the time flaky, to be
  fixed, never retried. Loosening the budgets is ruled out by ADR-0011.

## Decision

1. **Rounds spread over the job, gated on their median.** Cold start and the list take turns, `ROUNDS` = 5 times:
   each round 5 fresh servers (round value: the median of their cold starts), then 2 warm-ups and 12 reads by a fresh
   client (`client_id=budgets-r<n>`) of the idle server (round value: the median to the first page). The cold starts
   boot on a copy of the migrated home (`cold-home`), so the idle server keeps `home` and stays up between list
   rounds. The stream runs `PATCH_ROUNDS` = 3 times on the same server, each round 10 new tasks (their own gate file)
   and 4 new clients (round value: that round's p99 of 800). Each budget is gated on the median of its rounds (the
   upper middle of an even count). A slow spell spoils the rounds it overlaps, not the verdict.
2. **A calibration of the runner brackets each round.** Before and after every round, while the server is idle, the
   bench times a fixed workload in its own process: 200 one-byte round trips to an echo thread over loopback TCP (a
   thread wake-up each, as every request wisp answers is), then 4 passes of xorshift over a 64 KB buffer, each
   written to a file and read back; the median of 5 runs (7.2–9.7 ms on an M-series Mac). Nothing of wisp's runs in it,
   so it moves with the runner only. Its *usual* value is the job's fastest bracket (this runner undisturbed; no fixed
   number, so a Mac and a CI runner judge themselves). A round whose slower bracket is over `NOISY` = 1.5× the usual is
   *disturbed* (†). On Linux each round also reports the hypervisor's steal share (`/proc/stat`), not judged: the data
   a fixed per-runner reference would need.
3. **The verdict** (`Rounds::judge`, pure, unit-tested):
   - the gated value is the median of the clean rounds while they are most of them, else of every round;
   - **≤ budget → pass.** Noise only ever slows a timing, so a pass on a noisy runner is a real pass;
   - over budget with most rounds clean → **miss** (exit 1, as before);
   - over budget with most rounds disturbed → **too noisy to judge**: exit 3, an `::error title=Server budget: runner
     too noisy::` that names the budget, how many rounds, the calibration's worst ratio and the median, and says it is
     not a pass. A miss elsewhere in the run wins (exit 1). No rounds at all is never a pass; a calibration that never
     ran leaves no round clean, so over budget is *too noisy to judge*.
   Leaving a disturbed round out can't hide a regression: a slower wisp slows the clean rounds too, and the
   calibration has no wisp code in it to be slowed by one.
4. **The table shows every round** (a † on the disturbed ones), every restart's range, every read's spread, the pooled
   2 400 patch samples (min · p50 · p99 · max), and the calibration's usual and slowest values.
5. **Write amplification is the last stream round's** (its WAL pinned as before, ADR-0082); the other rounds don't pin.
   The glibc-defaults comparison server (ADR-0087) takes as many list reads as the gated idle server (71).

## Places it reaches (ADR-0040)

PR CI (the job summary's table, one `::error` per miss or per budget too noisy to judge) and anyone's terminal
(`cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml`). No user-facing surface.

## Edge cases and their tests

All in `spikes/server-budgets` (`cargo test --manifest-path spikes/server-budgets/Cargo.toml -p server-budgets --bin
server-budgets`), on the numbers above:

- The list's slow round is outvoted (all clean: 4.47 ms passes), left out when the calibration caught it (4 clean:
  4.47 passes), and the old single measure (7.87) missed:
  `a_round_the_runner_slowed_is_outvoted_and_the_old_single_measure_missed`.
- A list twice as slow misses on a quiet runner (8.94 ms), still misses with 2 of 5 rounds disturbed, and is *too
  noisy to judge*, never a pass, with 3 of 5: `a_list_twice_as_slow_misses_on_a_quiet_runner_and_is_never_passed_on_a_noisy_one`.
- Cold start on 25 of CI's restarts (424.5 and 226.2 among them) passes at 107.1 ms; the same +50 ms misses; db637559's
  restarts, before ADR-0084 made the start do less, miss:
  `cold_start_passes_on_ci_numbers_and_misses_50_ms_slower_or_on_the_build_before_adr_0084`.
- Patch p99 rounds 170.4 · 109.9 · 103.6 pass (109.9); +50 ms on every patch misses; two slow rounds of three miss:
  `patch_p99_rounds_outvote_one_stall_and_catch_50_ms_more_on_every_patch`.
- One stalled window (8 samples at 171 ms) is the p99 of 800 and isn't the p99 of 2 400:
  `one_stalled_commit_window_decides_a_p99_of_800_but_not_of_2400`.
- Every round disturbed but under budget passes; no rounds is an infinite miss; a usual of 0 or ∞, or a round with
  no calibration, is never clean: `noise_only_slows_so_a_pass_on_a_noisy_runner_is_a_pass_and_no_rounds_never_pass`.
- At the budget passes, 0.01 over misses; a bracket at exactly 1.5× is clean, above it disturbed:
  `at_the_budget_passes_and_the_noisy_threshold_is_exclusive`.
- A miss exits 1, a run too noisy to judge exits 3 with its reason in the message and ⚠️ in the table, and a miss
  wins over it: `a_miss_exits_1_a_run_too_noisy_to_judge_exits_3_and_says_why`.
- The calibration runs and takes time: `the_calibration_is_a_fixed_workload_that_takes_time`; the home's copy:
  `copy_dir_copies_folders_and_files`; a round's steal share, and none without two readings:
  `steal_is_the_share_of_ticks_between_two_readings_and_unknown_without_them`.
- ADR-0080's and ADR-0082's tests keep passing (each budget misses on its own, a never-delivered patch is a miss).
- A planted regression on a real run: `--wisp=<a script that sleeps 120 ms, then execs wisp>` (Mac, below).
- Checked at run time, every run: an earlier round's late writes (the pinned round waits for the WAL to stay unchanged
  1.5 s), an earlier round's stamps (only those written since the round began count), a gate file per round (an open
  one never releases the next round's tasks early).

## Consequences

- **Rounds are the fix; the calibration only names a failure.** The two misses above are one spoiled stretch each
  (30 reads in a row, one commit window), and spread rounds outvote them (tests above). The calibration decides
  whether an over-budget median is reported as a *miss* (exit 1) or *too noisy to judge* (exit 3); either way the job
  fails. It can't turn a failure into a pass.
- Mac (M-series, `budgets` profile, 2026-11-16, quiet, the final code): cold start 50.8 ms (rounds 48.3–52.0), the
  list 1.33 ms (1.29–1.35), patch p99 103.8 ms (103.8 · 103.2 · 109.3; 2 400 of 2 400 samples), calibration
  7.23–9.25 ms (1.3×, nothing marked), RSS 108.9 MB, write amplification 40.5×: exit 0 (two earlier runs of this
  change: 51.9 / 1.37 / 102.8 and 51.2 / 1.43 / 103.4). **A planted regression** (`--wisp=` a script that sleeps
  120 ms, then `exec`s wisp): cold start 181.3 ms, every round over (179.7–184.9): ❌, exit 1, the other budgets
  unchanged (list 1.41, patch 103.7).
- **What the calibration doesn't see, measured**: with 2 × 8 `yes` processes taking every core of the Mac for 20 s
  of the run, the list read 3.99 · 5.11 · 6.46 · 7.18 · 9.06 ms (a miss, exit 1) while a CPU-only calibration stayed
  at 6.7 ms; a second loaded run with the round-trip calibration read 7.5–8.7 ms against 7.4–9.7 quiet. macOS's
  scheduler runs the bench's short, often-sleeping threads ahead of the spinners (a standalone all-core spin wasn't
  slowed either), so in-OS contention on a Mac isn't a model of a CI runner, whose slow spells are its hypervisor
  taking the vCPU: that slows any workload, this one included. Unmeasured on Linux here (no docker): the table
  prints the calibration and the steal of every round, and the first CI runs show whether a spell moves them. Until
  then a spell the calibration misses ends as a miss (exit 1), as before, but only when it spoils most rounds.
- **The list has little headroom on CI's runner**: job medians 2.64–4.69 ms against 5 on the same code, so a runner
  10 % slower than these, all job long, misses. That isn't noise the gate should absorb. The list itself needs to get
  faster on Linux (plan §7).
- The calibration's usual is relative to the job: a runner uniformly slower than the last (an older CPU) isn't
  *noise* to it, and gets judged. A fixed reference per runner type would need its own ADR, with numbers from enough
  runs (the steal and calibration columns are that record).
- **Time**: the spike's run takes 52–53 s on the Mac (from `cargo run` to the table). On CI's runner, from the
  4612442b logs (each list read 110–240 ms to its last page, a restart ~0.2 s, a stream round ~12 s): about +40 s
  over the ~40 s the step's run took, so ~80 s, most of it the two extra stream rounds (~25 s) and the quiet wait
  before the pinned one (~2.5 s).
- Write amplification is the last round's, after the WAL went quiet for 1.5 s (≤ 15 s): what rounds 1–2's turn ends
  still write isn't counted. A stamp written before its round began (a row carrying an earlier round's text) is never
  a sample.
- Exit 3 is new: the step fails on it as on any non-zero exit (`.github/workflows/suite.yml` unchanged but for its
  comment). It means "this run couldn't judge", stated as such; it is never turned into a retry.
