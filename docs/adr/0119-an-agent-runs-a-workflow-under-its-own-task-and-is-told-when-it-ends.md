# ADR-0119: an agent runs a workflow under its own task, on any harness, and is told when it ends

- Status: accepted
- Date: 2026-11-25
- Source: owner, 2026-11-25: "wisp should have an equivalent for workflows that run inside Claude Code, for all harnesses,
  with a nice UI … check how agents can use it to run workflows inside a task"; "we should also have /workflow". The
  lead's research: Claude Code's Workflow tool (a lead agent writes a script of `agent()`, `parallel()`, `pipeline()`,
  `phase()` calls; it runs in the background, the lead is notified when it ends, `/workflows` shows it live), GitHub
  Actions (a DAG of jobs with `needs`, `if`, `matrix`, outputs), Temporal and LangGraph (durable steps, fan-out over a
  list known only at run time), and their UIs (a live graph or phase lanes, each step's state, time, cost and output, a
  click to its log). wisp already had ADR-0028's declarative runner, reachable from the CLI only.
- Changes: ADR-0028 (a run's parent), ADR-0115's agent-writes list (two commands added; the block below is the list now).

## Context

ADR-0028's `wisp.workflows` ran a saved workflow as a held parent task from `wisp run`. No agent could reach it
(`wisp.mcp` had no `workflows:*` grant, `agent:session` no `workflows:*` allow), it had no UI, every fan-out had to be
known when the workflow was written (`matrix`), and nothing told the task that asked for the run how it ended.
Claude Code's workflows are a Claude Code feature; wisp runs seven harnesses and every step can be any of them.

## Decision

1. **A run belongs to a task (its owner).** `workflows.run` takes `spec` (a workflow written for this run, not saved)
   or a saved `name`, and `task`. An agent's run is always its own task's; the user names a task or gets a held one
   (`wisp run`, as before). Each step is a child task of the owner (`parentId`, `lineage.workflowRun` = the owner,
   `ext["wisp.workflows"] = { run, node }`), so it shows as the owner's subtask like a delegate's.
2. **Where it stands lives on the owner**: `ext["wisp.workflows"].runs`, one view per run (each step's phase, title,
   agent, item, state, task, attempts, why, times, cost and a line of its output; the run's state, cost, budget,
   phases, done/total), the newest eight kept, written only when it changed. A held run task also carries the view at
   the top (what `wisp run --wait` reads). `runtime.summary` puts `workflowRuns` (and a step's `workflowStep`) on the
   list row, which the task page's band reads; nothing polls.
3. **The owner is told when it ends**: a queued `runtime.followUp` (`by: "workflow"`) with each step's result, so the
   agent that started it carries on, on any harness. Stopped runs and held owners send nothing.
4. **Fan-out decided at run time**: a step's `foreach: ${{ tasks.<step>.output.<list> }}` becomes one node per item once
   its needs end (`<step>[1]`, …; at most 64), its prompt reading `${{ item }}` and `${{ index }}`; an empty list skips
   it, a value that isn't a list fails it. `phase` and `title` are for the view. A step with an `output` schema is asked
   to end with an `output` block of that shape (the next steps read its fields). Nodes are ordered by depth.
5. **An agent's run stays inside its task's limits** (ADR-0112): its steps run in its task's folder on this computer
   (`project`/`on` refused), at its task's mode or lower (a step with none, or above, runs at the task's), and it stops
   or reads only its own task's runs. `workflows.set`/`remove` stay the user's setup.
6. **Agents get `workflows.run`, `workflows.status`, `workflows.stop`** (`agent:session` allows `workflows:read` and
   `workflows:run`; `wisp.mcp` holds `workflows:*`). `workflows_run`'s description is the authoring guide.
7. **`/workflow <words>`** (also `/wf`) in New task, a reply, the CLI, the phone or a channel makes the task's agent the
   run's lead: the prompt stays as typed (the task shows what the user wrote) and a `runtime.prompt` turn note in front
   of it (`PromptPart::before`) tells the agent to run a saved workflow (a saved name, with `k=v` inputs) or to plan one
   for the words and run it, naming the tool as that harness does; a harness without wisp's tools is told to do the job
   itself. `/workflow` alone is refused by `before:create`/`before:followUp`, saying what to type. wisp's own
   follow-ups are never read as one.
8. **A run cut by wisp stopping says so** at the next start (`stopped`, its working steps failed, the rest skipped);
   picking it back up is not yet.
9. Stop: `workflows.stop` (and the band's Stop) stops the working steps and skips the rest.

The agent's writes, exactly (this block supersedes ADR-0115's; `serve_mcp_tools` reads the newest one):

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
workflows.run
workflows.stop
```

## Places it reaches (ADR-0040)

Inside wisp (the task page's band, the list row), the user's harness and any task's agent (`workflows_run`,
`workflows_status`, `workflows_stop` over MCP), the CLI (`wisp run`, `wisp workflows …`), and every prompt box,
channel and phone (`/workflow`).

## Consequences

- Any harness can lead a multi-agent run and any step can be any harness (`agent:` or a `matrix` of them).
- A step's whole answer stays in its task; the owner gets a line of each (700 characters).
- Not yet: steps on another machine, resuming a cut run, enforcing an `output` schema (it's asked for, not checked).

## Tests

- `plugins/workflows/core`: `foreach_fans_out_over_an_earlier_steps_list_at_run_time`, `a_later_step_reads_every_foreach_run`,
  `an_empty_foreach_skips_and_a_non_list_fails`, `stopping_a_run_stops_whats_working_and_skips_the_rest`
- `plugins/workflows/host`: `an_agents_steps_never_run_above_its_mode`, `the_owner_is_told_each_steps_result`,
  `a_run_cut_by_a_restart_says_so`, `slash::reads_slash_workflow_and_nothing_else`,
  `slash::a_saved_name_runs_that_workflow_and_words_plan_one`
- `crates/wisp/tests/serve_mcp_tools.rs`: `an_agent_runs_a_workflow_under_its_own_task_and_is_told_how_it_ended`,
  `an_agents_capability_set_writes_only_its_own_work`, `every_write_an_agent_reaches_is_one_adr_0115_lists`
- `plugins/workflows/ui/test`: the band's model
