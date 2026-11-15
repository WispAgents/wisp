# ADR-0074: A write's events are queued before its rows can be read (`Ctx::outbox`, `Store::write_with`)

- Status: accepted
- Date: 2026-11-15
- Source: ADR-0073 "Not covered", first item: wisp.runtime saves a turn's end (`persist`) and publishes
  `runtime.turn.ended` only after that write returns (`emit_end`), so a client reading the store in between sees the
  turn ended, asks `budget.compare`, and `Ctx::events_delivered` can't wait for an event that doesn't exist yet.
- Amends ADR-0073 (closes that gap; its decision stands) and ADR-0044 (events published after commit: now *queued*
  before COMMIT and *let go* after it, for writes that carry an outbox). Additive to the kernel's plugin contract
  (`Ctx::outbox`, `Outbox`, `Staged`) and the store's API (`Store::write_with`).

## Context

- Inside a command, `ctx.emit` buffers on the root call; the events go out when the command returns `Ok`. Outside a
  command (a turn's runner, a timer, a watcher), `ctx.emit(&ctx.root_call(), …)` publishes at once. wisp.runtime
  emits every event that way (`CtxHost::emit`), after `store.write(..)` returned. Between the writer thread's COMMIT
  and the emit (the awaiting task being scheduled again) the rows are readable and the event doesn't exist.
- Publishing *before* the write would deliver a fact that may roll back; publishing *on the writer thread after*
  COMMIT still leaves a moment where another connection reads the row first. Only an event that is already in its
  queues when COMMIT makes the rows visible closes it, and then it must not be delivered until the write is known
  to have committed.
- The same pattern (write, then emit from a root call) is in other plugins: wisp.budget (`drain.rs`, `early.rs`),
  wisp.channels (`hub.rs`), wisp.machines (`commands.rs`, `manager.rs`), wisp.machine (`this_mac.rs`), and more
  (`grep "emit(&ctx.root_call()"`). None of those has a reader yet that answers from a subscriber's state right
  after the write the way `budget.compare` does.

## Decision

1. **`Ctx::outbox() -> Outbox`**: events collected for one store write (`Outbox::emit`, same `Event` types, same
   cause/root rules as `emit`). An outbox dropped without a write publishes nothing.
2. **`Store::write_with(outbox, f)`**: `f` runs in the transaction; then, still inside it, the outbox's durable events
   are appended to the event log (`Outbox::rows`, same transaction as the rows: no separate log write) and every
   event is **staged** (`Outbox::stage(head, seqs) -> Staged`): it takes its kernel seq and its place in every queue
   that wants it, **held**. Right after COMMIT, on the writer thread and before the change feed's listeners,
   `Staged::commit` lets them go (durable subscribers get the durable ones from the ring then); after a ROLLBACK (or a
   failed COMMIT, a failed op, a panic) `Staged::abort` drops them unseen. A `Staged` dropped unsettled aborts.
3. **A held event**: its queue's worker doesn't run its handlers and doesn't pass it (everything behind it waits:
   per-queue order is kept); `Bus::delivered` counts it (its seq is taken before COMMIT), so `events_delivered`
   from anyone who saw the write's rows waits for its handlers. A dropped one leaves its queue without its handlers
   running. Seqs are taken `AcqRel`.
4. **A durable repeat** (same type and key within 24 h, the log answers `None`) is not staged, as inside a command.
5. **wisp.runtime** writes a task through `write_with` (`Host::outbox`, `CtxHost` fills it): every task write carries
   its `session.changed`; the turn-end write (`end_turn` → `persist_emitting`) also carries `turn.ended`,
   `task.finished`/`task.failed`, and `turn.finished` when someone should hear, in that order (`end_events`). A test
   host (`Recorder`) returns no outbox: the runtime emits them after the write, as before, and the Recorder notes
   each write's events (`writes`).
6. Inside a command nothing changes: its events still go out when it returns `Ok` (or at the store's `CommitHook`).

## Edge cases (each with its test)

- A reader between COMMIT and the events going out asks a command that waits for its handlers: it waits and answers
  with them; no handler runs while held; the write's events arrive in emit order with the write's cursor; a
  rolled-back write's events are never delivered nor waited for; one dropped unsettled is the same —
  `crates/wisp-kernel/tests/kernel_events.rs` `a_writes_events_are_queued_before_it_commits_so_a_reader_of_its_rows_finds_them`
  (one thread, 200 chances to answer early).
- A real store: the commit listener (rows visible, the write not yet returned) asks; the old way (write, then emit)
  answers without the event, deterministically; through `write_with` it's there; the durable row is in the log from
  the same transaction; a failed write publishes and logs nothing —
  `crates/wisp-store/tests/durable_events.rs` `a_reader_who_sees_a_writes_row_finds_its_event_already_queued`.
- The write that ends a turn carries `session.changed, turn.ended, task.finished, turn.finished`; every task write
  carries its `session.changed` — `plugins/runtime/host/tests/lifecycle.rs` `a_turns_end_goes_out_with_the_write_that_ends_it`.
- Nine turns on two accounts, `budget.compare` asked right after the last is seen ended: both accounts with 5+
  turns — `crates/wisp/tests/serve_budget.rs` `where_the_plan_went_and_tokens_per_turn_by_account_come_from_real_runs`.

## Not covered

- The other plugins that emit after their own write outside a command (Context) still do; each moves to an outbox
  when something answers from a subscriber's state right after that write.
- A command's own events still go out when it returns, after its writes committed (`CommitHook`/`write_for` exist,
  nothing calls them): a reader inside that window misses them the same way. Closing it means every command's
  `store.write` staging the root's buffered events, a change to every command's timing of its own.
- A queue past `QUEUE_CAP` drops a staged event as it drops a published one (lagging, ADR-0044).

## Consequences

- A held event holds its subscribers' queues for the rest of the transaction (after `f`: the log append and COMMIT,
  microseconds). Nothing may await between staging and settling; the store stages last thing before COMMIT.
- The runtime's durable events are logged in the turn-end write's own transaction instead of a second one after it;
  a durable repeat is no longer delivered in process before the log says it's a repeat.
- wisp.runtime's events now go out at COMMIT, before `write_session` updates its in-memory list: a handler that
  reads `runtime.list` (memory) at once may see the row before it; `runtime.get` (the store) is current. Today's
  subscribers read the store or take ids only.
- The places it reaches: every surface that reads a turn's end and then asks about it (Settings → Usage, the picker,
  the CLI, MCP's budget tools) gets an answer that has that turn.
