# ADR-0120: a workflow shows as a row in its task's details, opens its board in the main area, and holds its task in Working

- Status: accepted
- Date: 2026-11-25
- Source: owner, 2026-11-25, on ADR-0119's band: "use the right space to show workflows … before the PR box in the
  right side … compact to match the details … when clicked it shows the workflow panel in the main area … navigation
  parent → workflow → sub task"; "the user should understand easily that there is a workflow working in the task";
  "check how agents check the status of tasks and events … make it perfect".
- Changes: ADR-0119 rule 2's UI (the band on `shell.task:band` is gone); how a run's task lands.

## Decision

1. **A row in the task's details.** `wisp.workflows.card` on `shell.task:details` with `order: -100` (first among
   the plugins' rows, over wisp.ship's PR card), as compact as the details' own rows: the kit's `RunChip`, the run's
   name and `done/total`, a bar of a segment per step run under it (a running one pulses), the phase, time and cost in
   its tooltip. One row per run going, else the newest; on a phone it sits over the turns as the details do.
2. **The board in the main area**: a `main` view (`workflows.board`, params `{ task, run? }`) with a task's
   head (the workflow's name, Back and Close to the task, Stop the run and Copy as workflow in its bar) and a
   breadcrumb (task › workflow), then the progress bar and the kit's `RunBoard`. A step card or a fan-out dot opens
   that step's task.
3. **Navigation is task → board → step and back.** A step's own task shows the same row (`Step <node> of <name>`),
   which opens its run's board; the board's Back and breadcrumb open the task; the browser's Back walks the same way.
4. **The task stays in Working while its run goes** (`runtime.after@1`, wisp.workflows.hold): an agent's turn that
   ends while a run of its task goes holds the task in Working (the list and the task say it works, never that it
   waits on you). The run's end lands it: wisp's message with the results starts the agent's next turn, or, with
   none to send (stopped, `notify: false`), `runtime.settle`. A follow-up moves the task on and the next turn's end
   holds it again; Stop on the task stops its runs.
5. **Agents and automations read it like any task's work**: durable `workflow.started` and `workflow.ended` events
   about the task (`events_tail`, `events_why`, triggers), `workflows_status` without the spec, and the task's own
   state (Working while it runs).

## Consequences

- No band over the turns; nothing new on the list's rows (their layout stays v1's).
- `runtime.after@1` has its first user: its `busy` line isn't shown anywhere yet.

## Tests

- `crates/wisp/tests/serve_mcp_tools.rs`: `a_task_stays_working_while_its_workflow_runs_and_lands_after_the_results_turn`
- `plugins/workflows/ui/test/board.test.ts`: the row's segments and line, the view's slot and params
