# ADR-0082: Patch latency, append rate and write amplification are gated in the server budgets' run

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *The server budgets in CI, step 2*. Follows ADR-0080 (its *Not covered yet*) and
  ADR-0011 (the budgets, plan §8).

## Context

- ADR-0080 gates cold start, idle RSS and the task list on a real `wisp serve` with 5 000 tasks. Three of
  ADR-0011's server budgets ran nowhere in CI: p99 patch latency ≤ 150 ms (one heavy user: 5k tasks, 10 running,
  4 clients), durable append ≥ 20 000 events/s, write amplification ≤ 15× payload.
- The append rate was measured by `spikes/events` (`append_rate`, run by hand), write amplification by
  `wisp-runtime`'s `#[ignore]` bench `write_amplification_2mb_session_in_5000_events` (one task, in process, a
  simulated CLI). Patch latency was measured nowhere end to end: `spikes/sync-bench-rs` times the sync fan-out only.
- Nothing could run 10 tasks that stream: the fake agent's runs print a handful of lines and end.

## Decision

1. **`FAKE_STREAM=<lines a second>`** in wisp-testkit's fake agent (as `FAKE_GATE`, `FAKE_HANG`): a claude run
   writes assistant text for `FAKE_STREAM_MS` (5 000) after its init, each line `FAKE_STREAM_BYTES` (400) long, its
   text starting `~stamp:<pid>:<n>:<unix µs>~`, the time it was written. Line n is due at start + n / rate, so a
   slow write never makes the stream fall behind. Detect probes (no prompt) stream nothing.
2. **Same run, same home, same table**: `spikes/server-budgets` runs it on the server whose idle numbers it just
   read. `claude` on the server's PATH is the fake agent (`--fake`, default `<target>/budgets/wisp-fake-agent`); the
   other CLIs stay version-only scripts.
   - *patch latency*: 10 tasks created with a `FAKE_GATE`d prompt, so they wait before their first line; 4
     clients, each with a hello `fresh=list,session:<each of the 10>` (the worst fan-out: G3's clients hold one
     open task each). Once every client has every first look, the gate opens; each client's first sight of a stamp
     in any frame is one sample (`now − stamp`, both on the machine's wall clock). 20 lines a second per task (10×
     G3's harness rate) for 6 s: 1 200 stamps, 4 800 samples. p99 ≤ 150 ms, and a stamp a client never got is a
     miss of its own.
   - *write amplification*: the store's WAL emptied (`wal_checkpoint(TRUNCATE)`) and pinned by a read from the
     spike's own connection while the stream runs (as the runtime bench pins it; SQLite's shared memory makes that
     hold across processes), then the database's growth once the server stopped. (WAL + growth) ÷ the JSON of the
     steps the 10 turns kept (`runtime.activity`, each `Step`'s `activity`: the runtime bench's payload).
   - *append rate*: `spikes/events`' `append_rate`, now its library, on a store of its own in the bench's folder.
     One measurement, two callers; `spikes/events` keeps running it by hand with its other budgets.
3. **One list of numbers**: `SERVER_BUDGETS` gains `patchP99Ms`, `appendPerS`, `writeAmplification`;
   `EVENT_BUDGETS`' two copies now read them. `tools/test/budgets.test.ts` compares all six with the spike.
4. CI builds `-p wisp -p wisp-testkit` in the `budgets` profile (the fake agent is a testkit bin).

## Places it reaches (ADR-0040)

PR CI (the job summary, an `::error` per miss) and anyone's terminal. No user-facing surface.

## Edge cases and their tests

- The fake streams at the rate asked, each line the length asked, stamps in order, and ends its turn; a probe
  streams nothing: `wisp-testkit` `the_fake_claude_streams_stamped_text_at_the_rate_asked`.
- A stamp sits anywhere in a frame (nested, JSON-escaped), or is cut: read, or skipped:
  `stamps_are_read_wherever_a_frame_carries_them_and_a_broken_one_is_skipped`.
- A client counts as ready only once each topic's last snapshot page is in:
  `a_topic_counts_as_looked_at_once_its_last_page_is_in`.
- A patch a client never got fails the run even when the rest are fast:
  `a_patch_a_client_never_got_is_a_miss_even_when_the_rest_are_fast`.
- p99 of 100 is the 99th; no samples never panic: `the_p99_is_the_99th_of_100_and_none_never_panics`.
- No payload kept is an infinite amplification, never a pass: `no_payload_is_an_infinite_amplification_never_a_pass`.
- Each of the six budgets misses on its own, and the budget itself passes:
  `each_budget_misses_on_its_own_and_the_budget_itself_passes`.
- A command's answer in chunks, or cut short: `a_whole_chunked_body_decodes_and_a_cut_one_is_an_error`.
- A task that never starts running, a turn that never ends, a client that never gets its first look, a WAL the
  server keeps busy for 2 s (each checkpoint try answers at once): exit 2 with the reason, the gate opened so no
  fake waits on (checked at run time, every CI run). A stamp no task wrote (n past the last) never counts.

## Consequences

- **The step stays red** on two budgets (2026-11-16, M-series Mac, `budgets` profile, five runs):
  - *patch latency*: p99 **301–303 ms** (budget 150), p50 156 ms, min 1.5 ms, max 304–310 ms, all 4 800 samples
    received. The path itself takes ~1.5 ms. The wait is a cadence: the store's `Coalescer` commits a task's
    activity 250 ms after its first buffered write (`CoalescerOptions::default`, plan §4.1; `drive.rs` never calls
    `step()` on a text line), then each client's outbox flushes at most every 50 ms (`flush_ms`). A uniform wait
    over 0–300 ms gives exactly this p50 and p99.
  - *write amplification*: **36.0–36.8×** (budget 15): WAL 12.4 MB + database 1.6 MB over 0.39 MB kept in 1 230 steps.
    The WAL held 3 159 page frames in 315 commits, about 10 pages (40 KB) a commit, against ~5 events (~1.6 KB) a
    commit: each of the 10 tasks' Coalescers commits on its own every 250 ms. By table: `change` 2.2 MB,
    `change_topic` 1.8 MB, `runtime_event` and its `turn` and `seq` indexes 5.3 MB, `sqlite_sequence` 1.2 MB (an
    AUTOINCREMENT bump every commit), page 1 (`sqlite_schema`'s root, the change counter) 1.0 MB. The runtime
    bench passes because one task streaming 400 lines a second puts 100+ events in each commit.
  - The two pull in opposite ways: a shorter window alone cuts latency and raises amplification. One commit for
    every running task each window (a shared coalescer or group commit), at ≤ 100 ms, gives both: ~10 commits a
    second instead of ~40 here. That is the next item, not this one; the budgets stay as they are.
- Cold start, idle RSS, the list and the append rate (~1 000 000 events/s) pass.
- The spike's run takes ~15 s on the Mac (was ~8 s): the stream adds ~7 s (6 s of lines, the tasks' start and the 4
  clients' 4.5 MB first looks), the 100 000 appends ~0.1 s. Compiling the spike takes ~6.5 s once `wisp` is built;
  the fake agent adds one small bin to the `budgets` build (its dependencies are `wisp`'s).
