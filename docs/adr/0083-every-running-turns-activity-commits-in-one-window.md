# ADR-0083: Every running turn's activity commits in one window (one shared coalescer, 100 ms)

- Status: accepted
- Date: 2026-11-16
- Source: docs/v2-plan.md §4.3 *Make patch latency and write amplification pass*. Follows ADR-0082 (its
  *Consequences*: the next item). Amends plan §4.1's write cadence (250 ms per task → 100 ms for all running tasks).
  Keeps ADR-0074/0075 (a write's events are queued before its rows can be read) unchanged.

## Context

- ADR-0082's gate (10 tasks streaming 20 lines a second to 4 clients, 5 000 tasks) missed two budgets: patch p99
  301–304 ms (150) and write amplification 36–37× (15).
- Each turn's drive made its own `Coalescer` (250 ms after its first buffered line). The wait a line sees is
  uniform over 0–250 ms, plus the client's outbox flush (at most every 50 ms): p99 ≈ 300 ms. Each of the 10 tasks
  also committed alone, ~40 commits a second, each paying the pages every commit writes (the change feed's tail,
  `sqlite_sequence`, page 1).

## Decision

1. **One coalescer for the runtime**: `Runtime::coalescer()` (made on the first drive, so its timer has the
   server's tokio runtime) is shared by every running turn. A window commits every turn's buffered lines in one
   transaction; the store groups appends per parent, so each task still gets its own change row (its topic), its
   rows in the order they came.
2. **The window is 100 ms** (`ACTIVITY_WINDOW_MS`, was 250 per task). A crash loses at most 100 ms of text.
3. **At once, without the window**: a tool's result (`ToolOutput`, `ToolError`, a `Tool` that carries its
   output) and a wait (`Effect::Waiting`) commit as soon as the driver hands them over; a turn's end flushes before
   its turn-end write (as before: `finish`), so its activity is never behind its end.
4. **A turn's wind-down flushes, never closes**: the buffer is shared; closing it would refuse the other turns.
5. **A refused shared batch never drops another task's rows**: if the store refuses the window's transaction, each
   buffered write commits alone (as before), and an append of several tasks' rows is retried one parent (task) at a
   time, so only the rows of a task the store refuses (a deleted one, its `runtime_event` foreign key gone) reach
   `on_error`, as one item. A task deleted while its CLI still runs costs each window one refused batch plus one
   write per task until the CLI is gone (≤ `LINGER`, 3 s); a store-wide failure costs one failed write per task.
6. The client's 50 ms outbox flush stays: at 10 commits a second the commits are ≥ 100 ms apart, so each one finds
   its stream idle and goes at once (`maybe_flush`); measured max 103–125 ms.

## Places it reaches (ADR-0040)

Every client that watches a running task (the UI, the CLI's follow, MCP's `wisp_session`, channels): lines show
within ~100 ms, not ~300. No new surface.

## Edge cases and their tests

- Three writers sharing one coalescer: one timer, one commit, one change row per task, each task's rows in order
  — `crates/wisp-store/tests/coalescer.rs` `several_writers_lines_commit_together_one_change_row_each_in_order`.
- A shared batch with one gone task's rows: the others commit (one change row per task still), only that task's rows are reported —
  `a_refused_row_in_a_shared_batch_never_drops_the_other_writers_rows` (fails on the old fallback: the whole
  append was one write).
- Two running turns, the store on a manual clock: their lines are buffered and nothing commits until the window
  passes, then one commit holds both tasks' lines, each in order; a tool's result commits with the clock standing
  still; both turns end with every step kept — `plugins/runtime/host/tests/group_commit.rs`
  `running_tasks_lines_commit_together_and_a_tool_result_or_a_turns_end_commits_at_once`.
- Existing runtime tests (lifecycle, delete_race, turn_window, report_back, trial, …) and the runtime's write
  amplification bench (one task, 2 MB in 5 000 events: 9.3×, 6.4×) pass unchanged.

## Consequences

Measured (2026-11-16, M-series Mac, `budgets` profile, `spikes/server-budgets`, three runs after, one before):

| budget | before | after | |
|---|---:|---:|---|
| cold start | 65.6 ms | 65.1–69.0 ms | ✅ |
| idle RSS | 100.7 MB | 99.1–105.7 MB | ✅ |
| task list, first page | 1.24 ms | 1.32–1.37 ms | ✅ |
| patch p99 (10 running, 4 clients) | 303.9 ms | **102.8–103.2 ms** (p50 52, max 103–125) | ✅ (was ❌) |
| durable append | 1 011 281 /s | 1 008 779–1 015 122 /s | ✅ |
| write amplification | 37.0× | **30.5–31.8×** | ❌ |

- **Write amplification still misses.** Commits fell 315 → 161 in the stream (69 of them activity), but an
  activity commit now writes ~28 pages: the structures keyed by task get one leaf per running task each
  (`runtime_event_seq` 6.3, `runtime_event_turn` 6.3, `change_topic` 5.2 a commit), and the shared tails
  (`change` 4.5, `runtime_event` 3.7, `sqlite_sequence` 1, page 1 0.9). The other 92 commits (turn ends, events
  log, cursors, router, budget: ~670 frames, 2.7 MB) are per turn, and the database grows 1.6 MB for 0.39 MB
  kept (each step is stored twice: its row and its change row).
- **Why a schema trim alone can't reach 15×** (from the WAL's frames, not a run): at 20 lines a second a task's
  steps fill ~2 pages a second, so any structure keyed by task that a 100 ms commit touches costs one 4 KB leaf per
  task per commit, ~5× on its own. Dropping `runtime_event_turn` (its reads would go by `(session_id, seq)` and a
  turn's first seq) and the change feed's AUTOINCREMENT (`sqlite_sequence`) would remove ~510 + 140 frames: ~24×.
  15× at this load needs one of: the change feed's append rows not repeating the row (resume reads the rows from
  their table), a longer window (latency misses), or a budget the gate's load can meet (an ADR that changes it).
  That choice is the owner's; this ADR changes no budget.
