# ADR-0073: A command that answers from its subscriptions waits for its own handlers first (`Ctx::events_delivered`)

- Status: accepted
- Date: 2026-11-15
- Source: PR CI run 836287a4, windows-latest node 24: `crates/wisp/tests/serve_budget.rs`
  `where_the_plan_went_and_tokens_per_turn_by_account_come_from_real_runs` saw its 9th turn end through
  `runtime.get`, asked `budget.compare` next and got `[]`. Every other job passed.
- Builds on ADR-0044 (events: published after commit, one queue per subscribing plugin). Additive to the kernel's
  plugin contract (`Ctx`).

## Context

- wisp.budget learns a turn's tokens (tokens per turn by account), its end (the ledger's sharers, Drain watch) from
  `runtime.turn.ended`, an ephemeral event delivered after the runtime's commit on wisp.budget's own queue. A client
  sees the turn end as soon as the runtime commits; the budget's handler runs later (it also writes the store and
  awaits `runtime.turn.setExt`), so on a slow machine a `budget.compare` or `budget.view` asked right after the turn
  ended answered without it: 4 of w1's 5 turns, below the 5-turn line, so no model at all.
- v1 answered from the sessions themselves (`compareAccounts(sessions)`): the turn a user just watched end was
  always in it. Settings → Usage opened right after a run would show it in v1 and might not in v2.
- Moving the count into the runtime's transaction would make wisp.runtime know the budget's ledger (ADR-0003); a new
  runtime query for every turn of the last 7 days would re-read what the budget already keeps in memory.

## Decision

1. `Ctx::events_delivered(within: Duration) -> BoxFuture<()>`: waits until the calling plugin's handlers have run on
   every event published before the call, at most `within`. An event stays in its plugin's queue while its handlers
   run; the wait returns when nothing queued has a seq up to the one taken at the call (the whole queue is looked at:
   two publishes from different tasks can queue out of seq order), and re-checks each time an event leaves
   (`Queue::popped`). With nothing queued it returns at once (one lock, no await). The plugin switched off ends the
   wait. Past `within` it returns anyway (a debug line): a handler stuck on something slow delays the answer, it
   never hangs it.
2. A command that answers from state its subscriptions fill awaits it first. wisp.budget's `budget.compare` and
   `budget.view` do, with 2 s (`EVENTS_WAIT`): its `runtime.turn.ended` handler is a few store writes, but it also
   sends a Drain watch notice through a channel, and the picker and Settings → Usage must not wait on a hung one.
3. It is a command's call, never an event handler's: a handler awaiting its own plugin's queue waits on itself. Nothing
   in wisp calls `budget.view` or `budget.compare` from a handler.

## Edge cases (each with its test)

- The handler is slow (blocked) when the command is asked: the command waits and answers with the event in it; with
  nothing queued it answers at once; a handler that never finishes holds it no longer than `within`, then it answers
  without that event — `crates/wisp-kernel/tests/kernel_events.rs`
  `a_command_waits_for_its_own_handlers_to_take_the_events_published_before_it` (one thread; the command gets 200
  chances to answer early before the handler is let go; without the wait it answers `0`).
- Nine turns end on two accounts and `budget.compare` is asked right after the last: both accounts with 5+ turns —
  `crates/wisp/tests/serve_budget.rs` `where_the_plan_went_and_tokens_per_turn_by_account_come_from_real_runs`.

## Not covered

- An event not yet published when the command is asked isn't waited for. wisp.runtime saves a turn's end
  (`persist`, a store write) and publishes `runtime.turn.ended` after that write returns (`emit_end`, outside a
  command's transaction), so a client reading the store in between (a scheduler wake-up of the runner task) sees the
  turn ended before the event exists; a command asked inside that gap answers without it. It's much narrower than
  the window this closes (the budget's whole handler: store writes, `runtime.turn.setExt`, a notice) but not closed:
  closing it means publishing the turn end in the write's own commit (the store's commit hook), a runtime change of
  its own. The same goes for a publish that took its seq but hasn't queued yet.
- A queue past `QUEUE_CAP` dropped events (lagging, ADR-0044): they're not waited for; durable subscriptions replay.

## Consequences

- A command using it waits as long as its plugin's handlers take on what is queued; a plugin's handlers should stay
  short (they already must: one queue per plugin).
- The places it reaches: every surface that asks these commands (Settings → Usage, the picker, the CLI, MCP) gets
  answers that include the turns the caller saw end.
