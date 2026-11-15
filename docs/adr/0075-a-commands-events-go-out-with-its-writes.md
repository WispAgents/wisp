# ADR-0075: A command's events go out with its writes (every `Store::write` in a command stages what it emitted so far)

- Status: accepted
- Date: 2026-11-15
- Source: ADR-0074 "Not covered", items 1 and 2: a command's own events still went out when it returned, after its
  writes committed (a reader in between missed them, as ADR-0074's runner did), and wisp.budget, wisp.channels and
  wisp.machines still wrote, then emitted on a root call.
- Supersedes ADR-0074's "Not covered" items 1 and 2 (its decision stands; its item 6, "inside a command nothing
  changes", is replaced by rule 1 here). Amends ADR-0044 (a command's events: published when the write that follows
  them commits, not when the command returns). The kernel's plugin contract is unchanged (`ctx.emit` on a command's
  call); the store's contract with the kernel changes (`durable::InTx::stage`, `committed`/`rolled_back` take
  `&self`) and the store's API grows (`Store::write_detached_with`).

## Context

- Inside a command `ctx.emit(&call, …)` buffers on the root (`RootTx::events`); `dispatch::run` published them after
  the command returned `Ok`. A command that writes and then awaits anything else (another command, a secret, the
  next write) leaves its rows readable for that whole time with its events not yet queued: `events_delivered` from a
  reader who saw the rows can't wait for them. The store already carried a command's *durable* events into the
  write's transaction (`durable::take`, ADR-0044), so the log had them while the in-process queues didn't.
- `CommitHook` / `Store::write_for` (publish at commit) existed, but nothing called them: every command would have to
  pick it. The fix belongs on the one write path every command already takes, `Store::write`.
- Outside a command, the pattern ADR-0074 closed for wisp.runtime was also in: wisp.budget (`drain.rs` Drain watch's
  note after `save`; `early.rs` `budget.reset` after `record`'s save), wisp.channels (`hub.rs` `mark_started`:
  `channels.startedFrom` right after a *detached* write, so it could even be delivered before the row existed), and
  wisp.machines (`State::publish`: `machines.changed` after a detached status write; `remove`: `machines.removed`
  after `forget`).

## Decision

1. **Inside a command, every `Store::write` (and `write_with`) stages the command's buffered events**:
   `durable::take()` is `Some` whenever a command runs (not only when it has durable rows); inside the transaction,
   after the closure, the log append and the cursor, just before COMMIT, `InTx::stage(head, seqs)` takes the root's
   buffer from its first event up to the first durable one this write doesn't log (one emitted *during* the write:
   it and what follows wait, so per-source order holds) and stages them as ADR-0074 does (held in every queue,
   counted by `delivered`, cursor = the change feed's head). Then the outbox's, if any.
2. **Settled on the writer thread with the write** (`Staged` carries an `Undo`): after COMMIT they go out and leave
   the buffer for good, and the durable ones this write logged reach durable subscribers (`Bus::remember`, as
   before); after ROLLBACK they're dropped unseen and put back at the front of the buffer, unlogged (`taken` and
   `written` reset), for the command's next write or its return.
3. **At return, unchanged for what's left**: events emitted after the command's last write (or by a command that
   wrote nothing) go out when it returns `Ok` (`append_rest`, `flush`); on `Err` they never do. So a command
   rejected before writing, or whose write rolled back and then failed, publishes nothing; a command whose write
   committed and that fails *afterwards* has already published that write's events (its rows are there: they're
   facts), not the ones it emitted after it.
4. **Which buffered durable events a write logs** is marked with that write's token (`Pending::taken: u64`), so two
   writes in flight for one command (a `join!`) never log or stage the same event twice and never mix up which
   answer is whose. The return's `append_rest` marks its own the same way, and it and `flush` leave events a write
   still in flight carries (one whose future was dropped, its job still queued): that write logs and stages them.
5. **Plugins that write then emit outside a command move to an outbox with their write** (ADR-0074):
   - wisp.budget: `State::save_with(outbox)`; Drain watch's `budget.drain` goes with the save of the spend that
     drained (`drain.rs`); `budget.reset` goes with `record`'s save (`reset_event`, then `reset_early` acts on it).
     With nothing dirty the outbox still goes, with an empty write.
   - wisp.channels: `kept::put_task_with(st, id, outbox)`: `channels.startedFrom` with the task-origin write
     (without a store the outbox comes back and the hub emits it itself).
   - wisp.machines: `machines.changed` with the peer's status write (`State::publish`), `machines.removed` with
     `forget` (`remove`); `machines.online`/`offline` ride an empty detached write (`State::emit_after_writes`) so
     they stay behind the `changed` of the status write just before them (per-source order).
6. **`Store::write_detached_with(outbox, f)`**: `write_detached` for callers that must not await (channels' hub, a
   peer's status), with ADR-0074's staging. A failed detached write publishes and logs none of its events.
7. **Not moved, on purpose**: `budget.planFree` (after `start`: no budget write of its own), `machine.loadFree` (a reading in
   memory, no row), channels' `changed()` (a transport's say-so, no write).

## Edge cases (each with its test)

- A command's events emitted before its write are queued held at the moment before COMMIT, delivered right after,
  while the command is still running; a reader asking then waits and answers with them (200 chances to answer
  early); emitted after its last write: out at its return with no cursor; write rolled back + command fails: none;
  write rolled back + command Ok: at its return, in order; write committed + command fails: the write's, not the
  rest — `crates/wisp-kernel/tests/kernel_events.rs` `a_commands_events_go_out_with_its_write_so_a_reader_of_its_row_finds_them`.
- A real store: the commit listener (row visible, `src.make` not returned) hears the command's durable events (2,
  then 4; the old way 0 and 3); a rolled-back write's command publishes and logs nothing; a command that writes
  nothing publishes at return; each durable row logged once — `crates/wisp-store/tests/durable_events.rs`
  `a_reader_who_sees_a_commands_row_finds_the_commands_events_already_queued`.
- A detached write with an outbox: the reader of its row hears its event; a failed one publishes and logs nothing —
  `crates/wisp-store/tests/durable_events.rs` `a_detached_writes_events_go_out_with_its_commit`.
- Drain watch: on the first commit whose account row shows the drain, a subscriber that waits for its handlers has
  heard `budget.drain` (the old way: 0) — `plugins/budget/host/tests/plan_free.rs`
  `drain_watchs_note_is_queued_before_the_drained_spend_can_be_read`.
- `machines.remove`: the reader of the deleted peer row has heard `machines.removed` (the old way: nothing) —
  `plugins/machines/host/tests/events.rs` `a_reader_who_sees_a_peer_gone_finds_machines_removed_queued`.

## Not covered

- `budget.reset`'s move, `channels.startedFrom` and `machines.changed` have no reader-at-commit test of their own
  (the first needs an early-reset reading timed on a commit; the others a transport and a live link); they go
  through the same `write_with` / `write_detached_with` the tests above cover.
- Writes a command makes without `Store::write`/`write_with` (`record_mutation`, `write_detached`, the coalescer,
  `write_blocking`) don't stage its events: those go out at its return, as before.
- A queue past `QUEUE_CAP` drops a staged event as it drops a published one (ADR-0044, ADR-0074).

## Consequences

- Every write inside a command now goes through `write_durable` and stages (one `Bus::stage`: the index snapshot,
  a gate, an enqueue per subscriber of each event): microseconds per write, on the writer thread, before COMMIT.
  A command's events reach their subscribers earlier (at the write, not at the return).
- A command's subscribers can now run while it is still running (after its write). A handler that reads the
  command's in-memory state at once may see it before the command finished updating it (wisp.machines' `remove`
  drops the peer from memory after `forget`); today's subscribers of the moved events read the store or take ids.
- Drain watch's note, `budget.reset`, `channels.startedFrom` and `machines.removed` are dropped when their write
  fails (before, they went out anyway): an event says what a committed write did.
- The places it reaches: every surface that reads a row and then asks a subscriber about it (the UI's topic then
  a command, the CLI, MCP tools, a channel reply) finds the events of the write that made the row.
