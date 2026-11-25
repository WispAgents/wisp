# ADR-0121: a workflow run changes while it goes, a failed step is told at once, and workflows are code with prebuilt starters

- Status: accepted
- Date: 2026-11-25
- Source: owner, 2026-11-25: "Agents have control of the workflow to cancel tasks, move tasks and add tasks?" → "Build
  workflows_edit for agents (skip, retry, add, change a step that hasn't started, concurrency and budget), the same
  actions on the board's step cards, and a message to the agent when a step fails"; "make sure the concept of everything
  as code applies here: the user should be able to export and import workflow.yml and start from a prebuilt workflow".
- Changes: ADR-0119's agent-writes list (`workflows.edit` added; the block below is the list now), how a run's failure
  propagates (ADR-0028).

## Decision

1. **`workflows.edit`** (`workflows_edit` for a task's agent, the board's step cards for you) changes a run while it
   goes, all or nothing, in this order: `change` (a step that hasn't started: the fields to set), `add` (steps, as in a
   spec), `concurrency`/`budget`, `skip` (the ones working stop; the steps after them run without them), `retry` (a
   failed or skipped step's tries start over, and the steps its failure skipped wait again). A target is a step's name
   (all its runs) or one run's id (`fix[2]`). Each change is checked as a workflow is at its start; a refusal says why.
2. The run's driver owns the run: a change goes to it over its control channel (with Stop) and is applied to a copy,
   kept only when every part took. Ended runs are kept (the last 16): a retry or an added step starts one again under
   its task (Working again).
3. **An agent's change is held to its task**: only its own task's runs, steps in its folder on this computer, at its
   task's mode or lower (`skipped by the agent` / `skipped by the user` says who, read the same by you and the agent).
4. **A step that fails for good while the run goes on is told to the run's agent at once** (a queued follow-up by
   `workflow`: which step, why, its task, and the `workflows_edit` call that retries or skips it). The run's end still
   sends every result.
5. **A failure carries down the chain**: a step skipped because one it needs failed keeps the steps after it from
   running too (before, the third step ran). A step you skip doesn't.
6. **Workflows are code** (ADR-0026): yours are the `workflow` kind (`workflows:` in a formula, `.wisp/*.wisp.yml`),
   imported with `wisp apply`, exported with `wisp export workflows`. **Prebuilt workflows** ship with the plugin
   (`starters.json`: audit, fix-failing-tests, two-ways, explore-plan-build, migrate) and run by name (`/workflow
   audit`, `wisp run audit`, `workflows_run {"name": "audit"}`) unless one of yours has that name; they are never in an
   export: `workflows.starters` lists them to start yours from. A workflow of one input takes the words after its name
   as that input (`/workflow two-ways make the parser faster`).
7. **A run becomes yours**: the board's Save as workflow keeps the run's workflow under its name (`workflows.set`,
   yours to export); Copy as workflow copies it as a formula.

The agent's writes, exactly (this block supersedes ADR-0119's; `serve_mcp_tools` reads the newest one):

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
workflows.edit
workflows.run
workflows.stop
```

## Places it reaches (ADR-0040)

The board (step cards' Skip and Retry, Save as workflow), any task's agent (`workflows_edit`, `workflows_starters`),
the CLI (`wisp workflows edit|starters`, `wisp apply`, `wisp export workflows`, `wisp run <name>`), every prompt box
(`/workflow <name> …`).

## Tests

- `plugins/workflows/core/src/edit.rs`: skip, retry (and what it reopens), add, change only before it starts, limits
- `plugins/workflows/host`: `an_agents_change_stays_at_its_mode_and_in_its_folder`,
  `a_skip_says_who_and_a_failure_message_says_how_to_act`, `every_prebuilt_workflow_checks`, the slash tests
- `crates/wisp/tests/serve_mcp_tools.rs`: `an_agent_skips_and_adds_steps_and_is_told_when_one_fails`
