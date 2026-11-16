# ADR-0080: The server budgets are gated in PR CI on a real `wisp serve` with 5 000 tasks

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *The server budgets measured in CI*, step 1 (r282's plan). Follows ADR-0011
  (performance is a feature) and ADR-0044 (spikes/events, the first gated spike).

## Context

- ADR-0011 gives the server budgets (plan §8) and says a miss blocks the merge, but only the bundle budgets
  (`tools/budgets.ts`) and the event log's (`spikes/events`) ran anywhere near CI. Cold start and idle RSS were
  measured nowhere; the list query only in-process (`wisp-runtime`'s `list_on_5k_sessions`, `rt.list`, not what
  the UI reads).
- The first measurement (2026-11-16, an M-series Mac, `budgets` profile, 5 000 tasks) misses all three:
  cold start **410–515 ms** median (budget 150), idle RSS **145–160 MB** after a client read the list (95 MB
  before; budget 120), the task list **~76 ms** median for a 4.5 MB snapshot (budget 5).
- The shipped release profile is fat LTO with one codegen unit: on the 8-core Mac `wisp` alone builds in about
  4 min in the thinner profile below, and fat LTO's single-threaded link would add several minutes to a CI job
  that already takes ~30.

## Decision

1. **`spikes/server-budgets`**, its own Cargo workspace (as `spikes/events`): the main `cargo build/clippy/test
   --workspace` never compiles it. It seeds a home, starts the real `wisp serve` binary and measures it from the
   outside; it exits 1 on a miss and 2 when it can't run (no binary, the server died, the list read the wrong
   number of tasks).
2. **What is measured**, on a home of 5 000 tasks seeded through wisp.runtime's own `schema::install` and
   `repo::save_session` (the fixture its list bench already uses; never a hand-made database):
   - *cold start*: process spawn → the first `200` from `GET /api/status`, median of 7 restarts, after one first
     boot (every plugin's migrations, reported, not gated);
   - *idle RSS*: the server's resident set (`/proc/<pid>/status` VmRSS; `ps` on macOS) once 8 samples 250 ms
     apart stay within 1 MB, after a client read the list; one that never settles in 30 s is a miss;
   - *the task list*: the UI's first read, `GET /api/events?…&fresh=list` (packages/sync's hello) → the last byte
     of the `list` snapshot frame, median of 30 after 3 warm-ups, one client id (one UI reading again). The client
     scans only the bytes each read adds (rescanning the growing frame timed the bench: +15 ms). A snapshot sent in pages is an error until the
     spike times every page.
   The environment is cleared: fake harness CLIs first on `PATH` and in every `WISP_*_BIN`, `HOME` under the
   bench's folder, `CI=1`, `DO_NOT_TRACK=1`, `WISP_TELEMETRY=0`, `WISP_UPDATE_CHECK=0`, no web build.
3. **One list of the numbers**: `tools/budgets.ts` `SERVER_BUDGETS`. The spike's constants must equal them
   (`tools/test/budgets.test.ts`), so a budget changes in one place, through an ADR.
4. **A `budgets` Cargo profile** (root `Cargo.toml`, copied setting for setting into the spike): `release` with thin
   LTO over 16 codegen units. The shipped fat-LTO binary is never slower than it, so a pass here is a pass there.
5. **CI** (`.github/workflows/suite.yml`, step *server budgets*): Linux, the first Node of the matrix, after the
   cargo and verify steps, into the job's target dir (the spike reuses the dependencies `wisp` built). It writes
   its table to the job summary and annotates each miss. Budgets are never loosened to make it pass.
6. Also printed (not gated): where the list's time goes, measured in process on the same home: the change-feed read
   `changed_since("list", 0)`, its JSON, and one query of the session table.

## Places it reaches (ADR-0040)

PR CI (the job summary, an `::error` annotation per miss) and anyone's terminal (`cargo run --profile budgets
--manifest-path spikes/server-budgets/Cargo.toml`). No user-facing surface.

## Edge cases and their tests

- The spike's numbers drift from `SERVER_BUDGETS`: `tools/test/budgets.test.ts`
  `budgets: spikes/server-budgets gates on SERVER_BUDGETS, the one list of the numbers`.
- The SSE body arrives in HTTP chunks split anywhere (mid size line, mid CRLF, mid frame):
  `spikes/server-budgets` `a_chunked_body_decodes_the_same_however_it_is_split`.
- The stream ends before the list snapshot: an error, never a hang: `the_last_chunk_before_the_snapshot_is_an_error_not_a_hang`.
- Medians over an odd count, and no runs at all, never panic: `spread_is_min_median_p95_max_and_never_panics_on_none`.
- A value at the budget passes, one unit over misses, each budget on its own:
  `each_budget_misses_on_its_own_and_the_budget_itself_passes`.
- RSS that never settles is a miss even under the budget: `an_rss_that_never_settles_is_a_miss_even_under_the_budget`.
- The server dies before answering, or the list snapshot has a different number of tasks than were seeded: exit 2
  with the server's log (checked by the spike at run time, every CI run).

## Consequences

- **The step is red today** on all three budgets, and stays red until they're fixed (where the time goes, measured
  2026-11-16):
  - cold start: `wisp.review`'s `on_start` lists every task and calls `runtime.get` once per task, in sequence,
    before the next plugin activates: ~360 ms of the ~400 ms. wisp.runtime loading the list takes ~25 ms.
  - the list: the `list` topic has no full-read snapshot source, so a first look replays the change feed
    (`changed_since("list", 0)`: ~54 ms for 5 000 rows) where one query of the session table takes ~4.5 ms; the
    rest (~17 ms) is the JSON, egress views, wire rows and framing of a 4.5 MB snapshot. 5 ms for every task in one snapshot
    also needs a smaller first page than all 5 000 rows.
  - RSS: 95 MB idle before any client, ~145 MB after list reads: the snapshot's transient allocations stay resident.
- The job builds `wisp` a second time (the `budgets` profile, besides the test build): several minutes on a cold
  cache, less once `Swatinem/rust-cache` holds the profile's dependencies.
- Not covered yet (plan §4.3, step 2): p99 patch latency (needs a FAKE_STREAM mode in the fake agent), append rate
  and write amplification in CI (measured by `spikes/events` and `wisp-runtime`'s bench, not in the PR job).
