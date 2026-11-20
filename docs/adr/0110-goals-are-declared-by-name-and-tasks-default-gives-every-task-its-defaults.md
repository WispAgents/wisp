# ADR-0110: goals are declared by name and tasks name them; `goals.default` and `tasks.default` give every goal and every task its defaults

- Status: accepted
- Date: 2026-11-19
- Supersedes: ADR-0028's `goal` singleton (the section `goal:` is now `goals:`; its one resource is `goals.default`)
- Source: owner, 2026-11-19 (`docs/owner-list.md` row 60): "In goals I expect the user to declare goals and reference
  them later on; also we have a `default:` in tasks which will have defaults for all tasks", and "when running a task
  the user should be able to attach the goal there: --goal <goal_id>"

## Context

Until now a formula had one `goal:` object (judges, vote, round limit: the defaults), and each task wrote its goal
inline (`goal: { text: … }`). A team that wants the same "done" for many tasks ("the build is green") had to copy the
text into each one, and nothing could start a task with a goal by name from the CLI, an agent or the app. Tasks had no
shared defaults either: every recipe repeated `agent: builder, mode: auto, worktree: true`. Neither was released, so
no compatibility is kept.

## Decision

1. **Named goals.** The `goal` kind (wisp.goal) has the section `goals:`, one resource per name
   (`goals: { green-build: { text, judges?, vote?, maxRounds?, sandbox? } }`). A goal must have `text`.
2. **`goals.default`** holds what every goal takes when it doesn't say: judges, vote, round limit, sandbox. It has no
   text, is never a goal anyone can name, and a goal's own fields win. It is what the singleton `goal:` was, under its
   new name (the stored row was already called `default`).
3. **Naming a goal.** A task's `goal:` takes a declared goal's name (a string) or an inline object (`{ text: … }`;
   `{ name: green-build, maxRounds: 3 }` is the declared goal with its own limit). On the wire (`runtime.create`'s
   `goal`) a name is `{ name: X }`. wisp.goal's `before:create` interceptor fills it in when the task is made (the
   goal's text, judges, vote, limit as declared now, `name` kept) or refuses the task with `no goal named X: goals has
   a, b`. A plain string that is exactly a declared goal's name is that goal too; any other string is what done means,
   as before (`/goal …`, `wisp_launch`'s `goal`).
4. **Checked before anything runs.** A resource kind may say what it names of another kind (`ResourceKind::refs`,
   additive, `wisp_resources_api::Ref`). The plan refuses a reference that names nothing as the apply would leave it
   (the formula's own goals and the ones stored), so `wisp validate`, `diff`, `apply` and Setup → Import all say
   `tasks.fix-build: no goal named X: goals has a, b`. A goal removed later leaves a task naming it; that task's next
   run is refused with the same words by rule 3.
5. **`tasks.default`.** The task named `default` gives every declared task its fields, under the task's own (a field
   the task gives wins, `worktree: false` included). It holds no prompt, title, steps, schedule or paused (each task's
   own; validate refuses them there), is stored like any task, and is listed only to the resource kind
   (`automations.tasks { withDefault: true }`): never a recipe, never run (`automations.task.run default` is refused),
   never scheduled.
6. **Who takes `tasks.default`:** every run of a `tasks:` entry, whoever starts it: by hand (`wisp run`, Recipes,
   `automations.task.run`), its schedule, or a trigger whose target is `task:`. A task started any other way (`wisp
   "…"`, New task, `wisp_launch`, a trigger's own `prompt:`, a crew member's job, a workflow step) doesn't: those
   don't come from a `tasks:` entry, and each has its own defaults (the `default` agent profile, Settings, the crew
   member's spec).
7. **Attaching a goal when running.** `wisp "…" --goal NAME` (a declared goal; an unknown one is refused) and
   `--goal-text "…"` (words); `wisp goal set <task> <name>` (`goal.set { name }`, or a `text` that is exactly a
   declared name), which carries the goal's round limit too (`GoalState.maxRounds`, additive); `wisp_launch`'s `goal`
   in both MCP servers (a declared name or words); and the app: Goal… in New task's and the reply box's `+` lists the
   declared goals by name (each with its text), then New goal…. A picked goal fills Done when with its text and goes
   by name while Done when still says that text; edited, it is the user's own words. `goal.declared` (read-only) lists
   the declared goals for these.
8. CRUD resource kinds now read back an optional bool's `false` (`worktree: false`): it was dropped as "empty", so a
   formula saying it planned a change on every diff.

## Consequences

- `goal:` in a formula is gone; `goals: { default: … }` replaces it (tests and guides moved). A task's `goal: "words"`
  string now means a goal's name: inline text is `goal: { text: … }`.
- A goal's text is copied into the task when it's made: changing a declared goal later changes the tasks made after,
  not the ones running (their goal is what they were started with; `goal.set` changes one).
- `refs` is generic: other kinds (a trigger's `task:`, a notification's `to:`) can declare theirs later; this ADR adds
  only the task → goal one.
- Not checkable by `tools/check` (a behaviour, not a structure rule): the tests hold it (below).

## Places it reaches (ADR-0040)

Formulas (`goals:`, `tasks.default`, `wisp validate`), the CLI (`--goal`, `--goal-text`, `wisp goal set`, `wisp goal
declared`), agents (`wisp_launch`'s `goal` in `wisp mcp` and in a task's own MCP server, `goal.declared` as a generated
tool), and the app (Goal… in New task and the reply box).

## Tests

- `plugins/goal/host/src/kinds.rs`: `default_is_the_defaults_and_every_other_name_a_goal`,
  `a_goal_needs_text_and_the_defaults_have_none`
- `plugins/goal/host/src/lib.rs`: `a_named_goal_is_the_declared_one_and_an_unknown_name_is_refused`,
  `goal_set_takes_a_declared_goal_by_name_or_by_its_name_as_text`
- `plugins/automations/api/src/tasks.rs`: `a_tasks_own_fields_win_over_tasks_default`, `a_tasks_goal_by_name_or_inline`
- `plugins/resources/api/src/adapters.rs`: `an_optional_bools_false_reads_back`
- `crates/wisp/src/scripted.rs`: `a_launch_takes_a_declared_goal_by_name_or_a_goal_in_words`
- `plugins/mcp/core/src/launch.rs`: `every_field_wisp_launch_documents_reaches_runtime_create_audit_c1` (goal)
- `crates/wisp/tests/serve_goals.rs`: `goals_declared_by_name_and_tasks_default_from_formula_to_run` (validate's
  unknown goal, apply, diff unchanged, a run with the defaults, `default` not run, `--goal`, `--goal-text`, `wisp goal
  set`, `wisp mcp`'s `wisp_launch`)
- `packages/shell/test/goal-declared.test.ts`, `plugins/goal/ui/test/judges-wire.test.ts` (the defaults are
  `goals.default` whatever else is declared)
