# ADR-0084: glibc's heap is trimmed after first looks, and the start does less before the first answer

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *Make the server budgets pass* (Linux). Follows ADR-0080 (the gate), ADR-0081 (the list
  in pages). Adds one reviewed OS call to `wisp-proc`'s sys module (plan §9.2: `unsafe` only there). No new dependency.

## Context

PR CI (ubuntu-latest, run `pr` on db637559) failed the server budgets on Linux only, where macOS passed:

| | Linux CI | macOS (M-series) | budget |
|---|---:|---:|---:|
| idle RSS, settled after 34 list reads | 297–300 MB (128–135 before the list) | 97 MB (91 before) | 120 MB |
| cold start → first response | 139–174 ms median | 67–70 ms | 150 ms |
| list, first page | 5.03–5.07 ms median (min 2.9, p95 8.6–10.8) | 1.3 ms | 5 ms |

No Linux machine or container was available to this change (no docker). Linux was reasoned from the CI log; the
slow-machine numbers below come from macOS with the server on its efficiency cores (`taskpolicy -c background`),
which reproduced CI's shape (cold 334 ms, list median 6.2 ms with min 2.9 and p95 15.7, every task 165 ms).

- **RSS is retention, not a leak.** The system allocator on macOS stays flat over 30 list reads (+7 MB, then flat).
  Each read parses its pages into `Value`s on whichever blocking-pool thread runs it and frees them on a worker; an
  allocator with per-thread heaps keeps that garbage. glibc keeps freed chunks in their arena and trims only an
  arena's top, so ~5 MB a read stayed resident (300 MB in CI), and the start's own garbage made Linux's idle 130 MB.
- **A global allocator makes it worse.** mimalloc 0.1.52 as the `wisp` binary's allocator, measured on macOS with the
  spike: its default (v3) line, idle RSS **188 MB** (97 with the system allocator), cold start 63 ms, list 1.30 ms;
  its `v2` line, **331 MB**, which is glibc's shape (139 MB before the list, ~7 MB more each read). jemalloc builds no
  MSVC target (the Windows app and npm package) and decays dirty pages over 10 s, longer than the gate's 2 s settle.
- **Moving the page parse to the sending side** (so values are made and freed on the workers) didn't help either:
  under mimalloc v2 the growth stayed (+9 MB a read), and with the system allocator every task went 40 → 245 ms (all
  11 raw pages reach the machine at once, past the high-water mark, and then wait a tick each). Not done.
- **Cold start** (macOS, debug trace of `activating`/`active`, then timers): of ~67 ms, `wisp.runtime`'s start took
  31 ms (12.8 ms after the parse fix below), `wisp.machine`'s 12 ms (`sysctl hw.memsize` and `date +%z`, spawned and
  awaited in its start), and every plugin start waits on the store's one writer thread, which a start loaded with
  42 harness rows: four detection rounds (runtime's own; router, telemetry and accounts asking `runtime.agents`),
  28 of those writes changing nothing but the row's `at`.
- `runtime.start` filled its list cache with `r.query("session", id+summary)` (each row's JSON parsed into a `Value`
  map), then `serde_json::from_value(v.clone())` per row: three passes over 2.5 MB. Text then one parse: 7.7 ms of
  its 11.5 ms is now `Summary`'s own `#[serde(flatten)]` (an api type, left as it is).

## Decision

1. **glibc's heap is trimmed after bursts of first looks** (`crates/wisp/src/trim.rs`). `wisp_proc::trim_heap()` is
   `malloc_trim(0)` on `linux` + `gnu` (glibc ≥ 2.8 releases the free pages of every arena, not just their tops) and
   `false` everywhere else (macOS, Windows, musl: their allocators give pages back). On glibc, `wisp serve` wraps the
   stream's `Snapshots` in `TrimAfter`: each first look (`since` none: a new client, a new epoch, a dropped cursor)
   wakes one trimmer when its last page has been handed over; the trimmer waits until no first look ended for
   `QUIET` (300 ms: a burst, every client's first look after a restart, trims once, and the last page has left its
   socket) and trims once on the blocking pool; a burst that never goes quiet (a busy grid box, a client stuck
   reconnecting) still trims `LONGEST` (5 s) after it began. One trim also runs 300 ms after the start (what the start
   freed). An idle server never wakes for it. Elsewhere the source isn't wrapped and nothing is spawned.
2. **The start does less before the first answer.**
   - `Repo::summaries` reads `query_json` text and parses each summary once from it (`rows::summaries_of`, borrowed
     `RawValue`s), skipping a missing or broken summary as `summary_of` does.
   - `wisp.machine` reads the machine's size and time zone in its sampling task, before its first sample, not in its
     start.
   - A harness row whose content (all but `at`) is what this process last stored isn't written again
     (`Rounds`' `Wrote.stored`); after a failed write it's unknown and the next round writes. `at` keeps its meaning
     ("when this row was written"). A Re-check still writes `checking`, then the answer.
3. **The list's first page is unchanged.** Its server work on the slow cores is ~3.5 ms (query 0.45, parse into
   values 1.2, the egress views 0.5–0.8, the frame 1.1–1.5); a handshake that held the second page's read until the
   first was handed over measured no difference through the E-core noise (the same binary's median moved 3.1–5.9 ms
   between runs), so it isn't in. Sending rows the views leave alone as their stored text (ADR-0081's *Consequences*)
   is what would cut it, and changes the snapshot path's types: a later ADR.

## Results (macOS, `budgets` profile, 5 000 tasks; Linux is PR CI's to prove)

| | before | after |
|---|---:|---:|
| cold start, P-cores (3 runs each) | 63.0–69.9 ms | 47.6–52.8 ms |
| cold start, E-cores (2 runs each) | 300.1–333.7 ms | 165.8–214.3 ms |
| idle RSS (macOS: no trim there) | 97.0–101.9 MB | 98.2–101.5 MB |
| list first page, P-cores | 1.31–1.34 ms | 1.31–1.35 ms |
| harness rows written by a restart | 42 | 14 |

On Linux the trim's effect is reasoned, not measured: a settled RSS of what's live (macOS: ~95 MB with the same
data) plus what glibc can't return (partly used pages). CI's cold start (139–174 ms) scaled by the after/before ratio
of the P-cores (~0.75) and the E-cores (0.50–0.71): ~70–130 ms, under 150 but not by a margin this change can show.

## Places it reaches (ADR-0040)

Every `wisp serve` on glibc Linux: the npm package's and the desktop app's Linux builds, grid boxes. No command, MCP
tool, wire message or setting changes. The `harnesses` topic carries fewer, identical-but-`at` updates.

## Edge cases and their tests

- A burst of first looks trims once, after it ends; an idle server never trims again on its own; a later read trims
  again: `crates/wisp/src/trim.rs` `a_burst_of_first_looks_trims_the_heap_once_after_it_ends_and_a_later_one_again`
  (tokio's paused clock).
- First looks that never go quiet still trim once 5 s after the burst began (fails without the cap):
  `first_looks_that_never_go_quiet_still_trim_once_the_longest_wait_is_over`.
- Only a first look wakes the trimmer (a resume, in pages or not, doesn't), and every read passes through unchanged:
  `only_a_first_look_wakes_the_trimmer_and_every_read_passes_through_unchanged`.
- Summaries from text equal the old per-row ones, a null, broken or missing summary skipped, bad text an error:
  `plugins/runtime/host/src/rows.rs` `summaries_read_from_text_are_the_rows_summaries_and_skip_the_same_broken_ones`.
- Rounds that find what the rows say write nothing, three at once included; a Re-check writes `checking` then the
  answer: `plugins/runtime/host/tests/detect.rs`
  `a_round_that_finds_what_the_rows_already_say_writes_nothing_and_a_recheck_still_shows_checking`; the topic's
  existing order, no-flash, newest-wins and hang-up tests in that file still pass.
- `malloc_trim` itself: one call, no pointer; compiled and clippy-checked for `x86_64-unknown-linux-gnu`, `-musl` and
  `x86_64-pc-windows-msvc`. Not run here (no Linux).

## Consequences

- Linux RSS after a burst of big reads drops back within ~300 ms instead of staying at its peak. The trim costs a walk
  of the arenas (milliseconds on hundreds of MB) and the next big read page-faults fresh memory.
- The trim is tied to first looks; other bursts (wisp.machine's `runtime.list` every 10 s, a long task's window) are
  left to glibc's reuse. If another path shows the same growth, it wakes the same trimmer.
- If CI's list first page still misses, the next step is the stored-text rows above, not a smaller first page.
