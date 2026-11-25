# wisp.workflows

Orchestration as code (ADR-0028). v1 had no workflows. Its closest things were a lead delegating to
subagents and the conductor's after-a-run pipeline.

## Intent

- A workflow is a DAG of task specs (`workflows:` in a formula). Each step has:
  - `needs` and `if` (`${{ tasks.tests.output.failed > 0 }}`);
  - `matrix` (one run per combination: `agent: [claude, codex]`);
  - `output` (from the step's answer: an `output` block, else the hero's status, stats, tests and text);
  - `retries`, `timeout` and `optional`.
- The workflow itself has `inputs`, `concurrency` and `budget` (`{ usd }`: no new step once it's spent).
  Prompts read earlier results with `${{ tasks.<step>.output.<field> }}`, `${{ inputs.x }}` and
  `${{ matrix.agent }}`.
- A definition is checked when it's applied: unknown names, cycles, and expressions that read a step they
  don't need.
- A run (`wisp run <name> [k=v…] [--wait]`) is a parent task, created held so it never runs an agent
  itself. Its children are its steps:
  - each child is made with `runtime.create`, with `parentId` and `lineage.workflowRun` set;
  - the runner wakes on `turn.ended`, with a 2 s look for timeouts;
  - each step's state is kept on the parent (`ext["wisp.workflows"]`);
  - the parent ends in review, or error when a required step failed.
- The decisions are pure (wisp-workflows-core): checks, expansion, what starts next, outputs.

## In a task (ADR-0119)

- An agent runs one from its task with `workflows_run` (`spec` written now, or a saved `name`); `workflows_status` and
  `workflows_stop` read and stop it. The user's `/workflow <words>` makes the task's agent the run's lead.
- A run belongs to a task: its steps are that task's children, its view is `ext["wisp.workflows"].runs` (the row's
  `workflowRuns`), and the task is sent every step's result when it ends.
- `foreach: ${{ tasks.<step>.output.<list> }}` fans out at run time (`${{ item }}`, `${{ index }}`); `phase` and
  `title` are for the board; an `output` schema is asked for at the end of the step's prompt.
- An agent's steps run in its task's folder at its mode or lower.
- UI (`ui/`): the band on `shell.task:band` (the kit's `RunBoard`: lanes, cards, fan-out dots), `/workflow` in the `/`
  menu with the saved workflows.

## Not yet

- `on:` another machine: steps run here, and the run says so.
- Picking a run back up after wisp stopped in the middle of it (it says it was cut).
- A schema-typed `output`: it is asked for, not checked.
- `on: { trigger }`: start runs by hand or from the CLI for now.
