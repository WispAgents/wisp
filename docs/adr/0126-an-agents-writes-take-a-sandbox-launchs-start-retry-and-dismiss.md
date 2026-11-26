# ADR-0126: an agent's writes take a sandbox launch's start, retry and dismiss

- Status: accepted
- Date: 2026-11-26
- Source: #926 (the owner's 2026-11-25 report: "/sandbox shows at once, signs Claude in from the task"), which gave
  an agent's capability set `sandbox.start`, `sandbox.retry` and `sandbox.dismiss` beside `sandbox.launch` without
  the list ADR-0115 keeps (audit I1).
- Changes: ADR-0121's agent-writes list (the three added; the block below is the list now).

## Context

ADR-0115 made every write an agent's token reaches one a fenced list names, and `serve_mcp_tools`
(`every_write_an_agent_reaches_is_one_adr_0115_lists`) reads the newest ADR's block. #926 split a sandbox launch in
two: `sandbox.start` answers at once and the launch goes on as a `sandbox.launch` row (preparing, sign-in, failed,
started); `sandbox.retry` tries a failed or waiting one again and `sandbox.dismiss` forgets a failed one. An agent
already had `sandbox.launch`, which does the whole launch in one call, so the test failed on the three new ids.

## Decision

1. An agent may reach `sandbox.start`, `sandbox.retry` and `sandbox.dismiss`: they start, try again or forget a launch
   that `sandbox.launch` (already an agent's) makes in one call, and change nothing else. A launch's own rules hold as
   they did (the grid's sign-in, the project it takes, the task it starts).
2. Nothing else changes. The agent's writes, exactly (this block supersedes ADR-0121's; `serve_mcp_tools` reads the
   newest one):

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
runtime.approval
runtime.appState
runtime.archive
runtime.ask
runtime.btw
runtime.btw.drop
runtime.card
runtime.compact
runtime.create
runtime.delegate
runtime.followUp
runtime.fork
runtime.forLater
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
sandbox.dismiss
sandbox.drop
sandbox.launch
sandbox.own
sandbox.place
sandbox.retry
sandbox.seed
sandbox.start
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
workflows.edit
workflows.run
workflows.stop
```

## Consequences

- An agent can watch its launch go on (the row) instead of waiting on one long `sandbox.launch` call, and retry or
  dismiss it the way you do on the task list.
- Places it reaches (ADR-0040): wisp's MCP for a task's agent and the CLI's catalogue; the UI already had them.
- Checked by `serve_mcp_tools` (`every_write_an_agent_reaches_is_one_adr_0115_lists`,
  `an_agents_capability_set_writes_only_its_own_work`).
