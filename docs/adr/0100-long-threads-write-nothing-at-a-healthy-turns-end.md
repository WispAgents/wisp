# ADR-0100: Long threads write nothing at a healthy turn's end

- Status: accepted
- Date: 2026-11-18
- Source: CI on baf6cebd: the server budget *write amplification* read 44.5× over its 44× (ADR-0088's ratchet). Follows
  ADR-0079 (Notes, health), ADR-0085 and ADR-0088 (the stream's write amplification, per-turn commits).

## Context

At every turn's end wisp.threads wrote twice, each in a transaction of its own, beside runtime's own turn-end commit:

- `threads_meta`: the task's turn count (`count_turns`), there only so a Notes write could say how old the Notes are.
  A task with no Notes got a meta row too.
- `threads_health`: the health worked out again, written whole. Its counts (`turns`, the context's share) move every
  turn, so the row changed every turn even while the thread was Fine and nothing showed it.

The bench's WAL (`spikes/server-budgets`, `--keep`, macOS) named them: `threads_meta` 10 commits / 43 frames,
`threads_health` 8 commits / 35 frames on its own plus a share of runtime's turn-end commits.

## Decision

- **No per-turn count.** A Notes write reads the task's turns at that moment (`runtime.turns`, its last turn's index +
  1) and keeps it as `threads_meta.turn`; the Notes' age is the turns now less that. Notes writes are rare; turn ends
  aren't. The `count` column stays (forward-only) and is no longer written.
- **Health is written when what it shows changes**: its level, its reasons and the error it repeats. The counts behind
  them are not compared. No row reads as Fine, so a healthy thread never writes one. Read and compare happen in the
  write's own transaction. `threads.health` works the health out from the task's turns when asked, so its counts are
  current even when the row is behind on them.

## Measured (`cargo run --profile budgets --manifest-path spikes/server-budgets/Cargo.toml -- --keep`, macOS)

| | write amplification | the stream's commits | frames | threads tables in the WAL |
|---|---:|---:|---:|---|
| before | 43.0× | 147 | 1 277 | `threads_meta` 20 frames, `threads_health` 30 frames |
| after | 39.8× | 125 | 1 173 | none |

Linux reads about 1.5 above macOS (ADR-0088), so CI goes from 44.5× to about 41×, under 44×. The budget is unchanged.

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| Three healthy turn ends write no meta and no health row; a worn one writes its health once; the same reasons a turn later write nothing; new reasons write; `threads.health` answers today's counts; Notes written now are 0 turns old | `plugins/threads/host/tests/notes.rs` `a_turns_end_writes_nothing_for_a_healthy_thread_and_health_only_when_it_shows_something_new` |

## Consequences

- The `threads_health` row on a task's topic can be behind on a count that nothing shows. Every number the Thread tab
  and the TUI show (context %, compactions, turns past 30, edits undone, the Notes' age) is also in one of the reasons.
  So when a shown number moves, the reasons change and the row is written: what they show is current.
