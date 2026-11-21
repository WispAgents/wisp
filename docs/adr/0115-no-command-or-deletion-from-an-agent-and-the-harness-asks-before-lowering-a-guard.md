# ADR-0115: no command or deletion from an agent, and the user's harness asks before lowering a guard

- Status: accepted
- Date: 2026-11-20
- Source: owner's delegate, 2026-11-20, on audit I (docs/qa/audit-I-orchestration.md): finding I1 ("an agent runs any
  shell command on the server through its MCP `runtime_exec`, out of Plan and out of Codex's read-only sandbox"; "an
  agent may `runtime_delete` its own task") and the report's Note on what the user's harness can switch off.
- Supersedes: ADR-0112 §3's list in four entries: `runtime.exec`, `runtime.delete`, `runtime.projectTest` and
  `machines.call` leave an agent's writes (the list below replaces §3's). ADR-0112 §1, §2 and §4–6 and ADR-0113 stand.

## Context

ADR-0112 §3 named `runtime.exec` among an agent's writes, though its own doc says it is the user's click on an `actions`
block. With its run's MCP token a Plan task (Claude Code's Plan runs no command; Codex's sandbox is read-only) ran `sh -c`
on the server as the user, in the user's checkout, and no one was asked. The same list let an agent delete a task, its
own included (its work and worktree went with it, or the worktree was left behind).

The report's Note: the user's harness (`wisp mcp`, the HTTP MCP as the user) acts for the user (ADR-0040), so it holds
`settings_set` and `worktrees_trust`. `settings_set {"key":"worktrees.projectAsk","value":"off"}` turned off "Ask before
trusting a new project's setup scripts" with no confirmation. Any session in the user's harness, a prompt-injected one
included, could switch off wisp's own safety asks unseen.

## Decision

1. **`runtime.exec` is the user's click only.** `agent:session` (and every set built from it: `agent:session:<mode>`,
   `agent:capped*`, `crew:<name>`) denies `runtime:exec`, so it is neither listed in a run's MCP nor callable with its
   token; the refusal says the command runs when the user clicks it. Default deny holds whatever the task's mode.
2. **An agent deletes nothing.** `agent:session` denies `runtime:delete`: no task, its own or a child it started, is
   deleted from an agent's token (its work and worktree go with it). An agent archives, stops or marks done instead.
   - **Nor runs code another way** (the review of this fix): it may not set a project's test command
     (`runtime:projectTest`: a factory's checks run it without asking), nor call another computer's wisp directly
     (`machines:call`, a raw proxy); and a command it sends to another computer (`machines.route`, a tool's `peer`) is
     held to what its own set may run here, since it runs there as this computer's link.
3. **An agent's writes are exactly this list** (`serve_mcp_tools::every_write_an_agent_reaches_is_one_adr_0115_lists`
   reads it from this file, and `an_agents_capability_set_writes_only_its_own_work` holds it in the code): every write
   an agent's capability set reaches, and every write behind a tool its MCP lists (besides the setup asks of ADR-0113),
   is one of these. A new one is a decision in a later ADR.

```agent-writes
connectors.call
crew.agent.kit
crew.ask
crew.assign
crew.checkin
crew.export
crew.reply
crew.wake
crew.wakeMentions
git.checkout
git.clone
git.commit
git.restoreTurn
git.revert
git.stage
goal.check
goal.drop
goal.pause
goal.resume
goal.set
history.resumeRequest
later.add
later.agent.schedule
later.cancel
later.cancelPlan
later.done
later.edit
later.editPlan
later.park
later.plan
later.tick
machines.bring
machines.carry
machines.dropCopies
machines.route
machines.send
machines.uncarry
mcp.rpc
runtime.agent.tag
runtime.agent.task
runtime.agent.watch
runtime.answer
runtime.appState
runtime.approval
runtime.archive
runtime.ask
runtime.btw
runtime.btw.drop
runtime.card
runtime.compact
runtime.create
runtime.delegate
runtime.followUp
runtime.forLater
runtime.fork
runtime.held
runtime.markRead
runtime.note
runtime.pin
runtime.probe
runtime.projectRun
runtime.queue.cancel
runtime.queue.edit
runtime.queue.merge
runtime.queue.order
runtime.queue.sendNow
runtime.queue.text
runtime.quick
runtime.quickCancel
runtime.release
runtime.rename
runtime.rerun
runtime.restoreSession
runtime.reveal
runtime.setAgent
runtime.setLean
runtime.setMode
runtime.setState
runtime.settle
runtime.setup.ask
runtime.stop
runtime.terminal
runtime.unask
sandbox.bring
sandbox.drop
sandbox.launch
sandbox.own
sandbox.place
sandbox.seed
ship.agent.pr
stuff.addStarter
stuff.agent.save
stuff.launch
stuff.receive
stuff.remove
stuff.removeUpload
stuff.resetData
stuff.restore
stuff.reveal
stuff.save
stuff.stop
stuff.update
threads.fresh
threads.notesUpdate
```

4. **The user's harness asks before it lowers a safety guard.** A write from an MCP caller that is the user (no task's
   token) and lowers one of wisp's guards doesn't run: `wisp-mcp-core::guard::lowers` decides, and wisp.mcp makes a
   task titled "Your harness wants to …" that asks the user (Allow / Deny, shown wherever a question is: the task page,
   the phone, channels, `wisp reply`), held so its prompt never runs. The harness's call waits as `ask_user` does. Allow
   runs exactly the write shown, as the user; Deny, a typed answer or no answer changes nothing and the harness reads
   so. The task is done after. While it asks, the harness may read that task but not answer, reply to, stop or change
   it ("That task asks the user, not their harness").
   - The guards: `worktrees.projectAsk` off, `worktrees.worktree` off (tasks in the checkout itself), `git.maskEnvFiles`
     off, `automations.webhookTriggers` on, `channels.acceptChannels` on, `budget.limitHold` later or off,
     `machine.batteryHold` lower or off, `runtime.setupAnswerSets` with a set added, and `worktrees.trust` (a repo's
     own programs run without asking).
   - **And what runs code without one of wisp's asks**, always: `runtime.exec`, `plugins.install`, `plugins.trust`,
     switching a plugin on, `machines.call`, and `machines.route` of any of these.
   - A value is read as the kernel stores it (`false` or `no` for an on/off setting is `off`). The user's set is
     `user:*`, or `box` on a grid box.
   - A guarded write is never sent to another computer with a tool's `peer` (its value there isn't known here): it's
     refused, saying to change it on that computer.
   - The question is answered only by the user's own hands (`runtime.answer` from `user:*`/`box`, or a channel
     relaying their tap); a follow-up never answers it, from anyone (a `later.add` scheduled by the harness would have
     answered "Allow"), and an agent's token answers only its own task's questions (the kernel's agent scope).
   - **Raising a guard, keeping it, or any other setting applies at once**, as before (ADR-0040).
   - The app, the CLI and the phone write settings as before: they are the user's own hands.

## Consequences

- A user who wants an agent to run a command uses the `actions` block's button (the agent proposes it) or their own
  terminal. A lead that wants a child gone stops or archives it; the user deletes.
- The guard card is not a boundary against a harness that has a shell as the user (ADR-0113's threat model): such a
  harness can run `wisp settings set` or `wisp reply` itself. It stops a prompt-injected session that only has wisp's
  MCP tools from switching off wisp's asks unseen. A guard task's open question is kept in wisp.mcp's memory while the
  call waits; after a restart its Allow runs nothing (the call that would run it is gone) and says so as any answered
  question.
- Connectors the user's harness adds (`wisp_connector_add`, v1's feature, ADR-0112/0113) stay direct: they are the
  user's own setup tool; their commands run for every task, which the user's own harness is trusted to add.
- Reaches (ADR-0040): the user's harness (MCP), the app, the phone and channels (the question), the CLI (`wisp reply`).

## Edge cases

| Case | What happens | Test |
|---|---|---|
| A Plan task calls `runtime_exec` on its own task | not listed; refused, nothing runs | `crates/wisp/tests/serve_mcp_tools.rs` `an_agent_can_neither_run_a_command_through_runtime_exec_nor_delete_a_task` |
| An agent deletes its own task | refused; the task stays | same |
| A command newly exposed to agents | the list test fails until an ADR names it | `every_write_an_agent_reaches_is_one_adr_0115_lists` |
| The harness turns `worktrees.projectAsk` off | a task asks; nothing changes until Allow | `the_users_harness_lowers_a_safety_guard_only_on_the_users_allow` |
| The harness answers its own guard card | refused | same |
| Deny | nothing changes; the harness reads that the user said no | same |
| The harness turns a guard back on, or changes `crew.tint` | applies at once | same; `plugins/mcp/core/src/guard.rs` `raising_or_keeping_a_guard_and_other_settings_apply_at_once` |
| The harness sends `{"value": false}`, or a guarded write with `peer` | asks; refused for a peer | `guard.rs` `turning_a_guard_off_from_the_harness_needs_the_users_allow`; `serve_mcp_tools` `the_users_harness_lowers_a_safety_guard_only_on_the_users_allow` |
| A follow-up (or a scheduled one) to the asking task | refused, nothing answered | `the_users_harness_lowers_a_safety_guard_only_on_the_users_allow` |
| An agent routes `runtime.exec` to another computer | refused | `an_agent_can_neither_run_a_command_through_runtime_exec_nor_delete_a_task` |
| Each guard lowered | needs the user's Allow | `guard.rs` `turning_a_guard_off_from_the_harness_needs_the_users_allow` |
