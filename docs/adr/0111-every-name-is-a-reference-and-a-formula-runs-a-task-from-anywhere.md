# ADR-0111: every field that names a thing is a reference checked the same everywhere, and any launch can run under a formula

- Status: accepted
- Date: 2026-11-20
- Source: owner, 2026-11-19 (`docs/owner-list.md` row 61): "Cover these cases: I want `wisp "prompt" --formula`, and do
  the yml declare files use references?" and "Make sure everything can be atomic and referenced." Audit:
  `docs/qa/refs-audit.md`.

## Context

Every resource was already atomic (declared on its own, addressed `<kind>.<name>`), but references worked in one place
only: a `.wisp/` folder evaluated `${kind.name}`, variables and modules (ADR-0034); a single `-f` file or `--formula`
was read raw, so `${agent.critic}` reached the store as text and `variables:` was refused. Of the ~40 fields that name
another resource, the plan checked one (a task's `goal`, ADR-0110): a trigger naming no task, a crew member delegating
to nobody, a notification to a channel that isn't there all applied and failed later, or silently did nothing. And a
formula could run one task only as `wisp run --formula f.yml "…"`: `wisp "…" --formula f.yml` made `--formula f.yml`
part of the prompt, `wisp run release --formula f.yml` sent `release` to an agent as the prompt, a task file could only
carry an inline copy of its formula, and neither MCP server nor New task took one.

## Decision

1. **A kind says which of its fields name another kind** (`KindInfo::names`, additive): a path into the spec
   (`agent`, `connectors[]`, `members.*.agent`, `team[].crew`) and the kind it names. Only strings are names: an inline
   object, a URL, a path or an unevaluated `${…}` isn't, and a field may skip its own words (a notification's events).
   A kind with a name to check that a path can't say (a setting's value, a `tools:` entry's own name) keeps
   `ResourceKind::refs` (ADR-0110).
2. **A field may name what isn't a resource** (`ResourceKind::also_named`, additive): an agent field names a declared
   agent, a harness or `auto` (`CrudKind::also_named` lists `runtime.agents`); a machine field a peer added in the app
   or `this`.
3. **The plan checks every one** wherever a formula is read (`wisp validate/diff/apply`, Setup → Import, a grid
   deploy): a name that names nothing as the apply would leave it is refused, `tasks.t: no agent named critc: agents has
   critic (or claude, codex, …, auto)`; removing a resource something still names is refused too. A field marked
   `acyclic` refuses a loop (`crew.team[].crew`: `a loop: crew.Ada → crew.Bo → crew.Ada`).
4. **Every formula evaluates the same** (`module::evaluate`): a `-f` file, `--formula`, a task file's formula and a
   picked file evaluate `${…}`, `variables` (`-var`, `--var-file`, `WISP_VAR_<declared>`), `locals`, `count`,
   `for_each`, `outputs` and `modules` as `.wisp/` does. An unknown `${agent.nope}` says what there is; a module's
   resource is referenced by its address (`${module.team.agent.reviewer}`, its stored name); a module that uses itself
   (by its files' content, however its source is spelled) is refused.
5. **One task under a formula is resolved by one command**, `resources.overlay` (new wire message, read-only,
   `resources:read`): given resources already read, a formula's or a task file's text, or files on this computer, it
   answers the task's overlay (its agents, projects, harnesses, skills, teams, connectors), the task resolved (a
   declared task by name with `tasks.default` under it and its `{params}` filled; a goal the formula declares inlined;
   one declared agent the task's own) and the names it gives that name nothing, with what there is. The CLI reads files,
   URLs and gists itself and sends resources; New task sends a picked file's text; a task's `wisp_launch` names files
   in its folder (read off the runtime, on wisp.mcp's own grant). Nothing is applied.
6. **Where it reaches:** `wisp "…" --formula f.yml[,g.yml]` (a plain launch, `--compare` included; `--agent` may
   name the formula's agent); `wisp run <task> --formula f.yml [k=v…] [words…]` (the formula's task by its name, else the
   words are the prompt); a task file's `formula:` takes a file beside it, `module.x` (a module of its folder's `.wisp/`,
   its resources under the names they have inside it), a formula inline, or a list (a file named twice, or the task file
   itself, is refused); `wisp_launch`'s `formula` in `wisp mcp` and in a task's own MCP server (whose `agent` now takes a
   declared agent, not only a harness); New task's `+` → Formula or task file… (loaded on press: the first screen gains
   one row, one chip and one spread).
7. **A task's folder may be a project's name** (`project: web` in a declared task, `cwd: web`): `runtime.create`
   resolves it to that project's path (the overlay's or the stored one, any case) before anything reads the folder, or
   refuses it naming the projects there are.

## Consequences

- A formula that applied before may be refused now, naming what it gives that names nothing (every such formula was
  broken at run time already: a trigger whose task isn't there never ran).
- `resources.overlay` (permission `resources:overlay`, granted to `agent:session` and declared by wisp.mcp) reads with
  the caller's own reach (review r194 #1): the user reads any file on their computer, as the CLI does; anyone else only
  inside its own task's folders (its folder, its worktree, that worktree's checkout), by relative path, no `..`,
  resolved after links, never wisp's home, `secrets.json` or `.env*`, and never `file()` or `${env:…}`. A caller with no
  task reads no files. wisp.mcp's `wisp_launch` calls it as the asking agent, never as the kernel.
- **An agent's formula** (any caller of `resources.overlay` but the user, the kernel and a grid box's owner; and any
  `formula` an agent's token passes to `runtime.create`) declares only agents (without an account), skills (as text,
  never a path), teams and goals (`AGENT_FORMULA_KINDS`): no connector or harness (each runs a command on the host), no
  project (its setup runs), crew, trigger, task or anything else; no `${secret:…}` anywhere; and every name its
  resources give (an agent's connectors and skills) must name one the user set up. It never carries a `task:`: from an
  agent's `wisp_launch` only the agent and goal the formula resolves are taken. Each file it reads, a module folder's
  included, is resolved and kept inside the task's folders (review r194 round 2 #1, #3, #5).
- **A task an agent starts gets nothing its own task may not have**, checked once, in `runtime.create`, whenever the
  caller is an agent's token (`wisp_launch`, a raw `/api/cmd`, a plugin that kept the agent's call such as a crew job):
  a mode above the parent's own or its cap is refused, one from a profile or the default held down to it; no connector,
  crew member, account or named team the parent lacks; no team row on another machine; no `confirmLimit`, `profile`,
  `ext`, `worktreeKey`; and the parent's channel cap travels to the child. An agent's own follow-up can't lift its
  task's mode or skip Ask near the limit. `wisp_launch` to another machine is refused while the parent is held below
  Full auto (that machine's `runtime.create` doesn't see the agent). Its token reaches no other task (the kernel's
  scope), so fork and follow-ups touch only its own; side discussions are the user's (review r194 round 2 #2, #4).
- A grid box's owner (`box`) reads files as the user does, but never wisp's home or a secrets file (round 2 #7).
- The agent's own capability set (what it may write at all, and that it never raises a mode, its own task's included)
  is ADR-0112 (review r194 round 3): the parent check here and that set together are what hold a task's agent.
- Only a formula's own namespaces, as it uses them, are expressions: `${<kind>.<name>…}`, `${var.x}`, `${local.x}`,
  `${module.m…}`, `${count.index}` and `${each.key|value}` inside a resource that counts, `${workspace}`, a function called
  (`${upper(…)}`), `${secret:…}` and `${env:…}`. A shell's `${HOME}`, GitHub's `${{ … }}`, `${1}`, and a bare
  `${project}`, `${task}` or `${count}` in a prompt are kept as written; `$${x}` is always `${x}` (review r194 #2, #3,
  round 2 #6).
- A goal from a formula that isn't applied is copied into the task (as a declared goal is when it's made): it goes by
  its text, not its name.
- Not checkable by `tools/check` (behaviour, not structure): the tests below hold it.

## Places it reaches (ADR-0040)

Formulas (`wisp validate`, `.wisp/`, `-f`), the CLI (`--formula` on a launch and on `wisp run <task>`, task files), agents
(`wisp_launch`'s `formula` in `wisp mcp` and in a task's own MCP server), and the app (New task's Formula or task file…).

## Tests

- `plugins/resources/api/src/lib.rs`: `a_name_field_finds_its_names_by_path_and_only_names`
- `plugins/resources/host/tests/names.rs`: `every_name_field_is_checked_unknown_names_say_what_exists_and_loops_are_refused`,
  `a_task_files_formula_is_referenced_and_its_names_resolved_against_it`
- `plugins/resources/host/src/overlay.rs`: `a_declared_task_by_name_takes_the_defaults_and_its_params`
- `plugins/resources/core/src/module.rs`: `a_reference_names_what_exists_a_modules_resource_by_address_and_a_module_never_uses_itself`,
  `a_single_files_formula_evaluates_as_a_folder_does`
- `plugins/resources/core/src/overlay.rs`: `a_formula_is_a_tasks_overlay_and_says_what_it_leaves_out`,
  `a_task_files_formula_is_files_modules_or_inline_and_never_loops`, `a_modules_resources_keep_the_names_they_have_inside_it`,
  `a_goal_by_name_is_the_formulas_goal_inlined`, `a_tasks_names_are_checked_and_unknown_ones_say_what_there_is`
- `plugins/runtime/host/src/specs.rs`: `a_projects_name_is_its_folder_and_an_unknown_one_says_which_there_are`
- `crates/wisp/tests/serve_formula_refs.rs`: `a_formula_runs_everywhere_and_every_name_in_it_is_referenced` (validate's
  references and refusals, settings and `tools:` names, `wisp "…" --formula`, `--agent` from the formula, `wisp run
  <task> --formula`, a task file's `formula: ./f.yml`, its unknown name and a loop, `wisp mcp`'s `wisp_launch`)
- `crates/wisp/tests/serve_mcp_tools.rs`: `a_tasks_wisp_launch_runs_under_a_formula_and_refuses_a_name_it_lacks`
- `packages/shell/test/formula-pick.test.ts` (New task's Formula or task file…, the project-folder note)
- review r194: `crates/wisp/tests/serve_mcp_tools.rs` `an_agents_formula_reads_only_inside_its_own_folder`,
  `a_child_never_gets_a_higher_mode_than_its_parent`; `plugins/resources/core/src/module.rs`
  `only_a_formulas_own_names_are_expressions_the_rest_is_kept_as_written`, `the_escape_always_comes_out_as_a_literal`,
  `a_self_reference_says_it_refers_to_itself`; `plugins/resources/host/src/lib.rs`
  `a_large_crew_graph_is_checked_for_loops_in_linear_time`; `plugins/resources/host/tests/names.rs`
  `a_task_files_formula_is_read_from_its_own_folder_or_the_project_said`
- review r194 round 2: `crates/wisp/tests/serve_mcp_tools.rs` `an_agents_formula_runs_nothing_reads_no_secret_and_carries_no_task`,
  `a_child_started_by_an_agents_token_gets_nothing_its_parent_lacks`, `a_capped_parents_cap_travels_to_its_child`;
  `plugins/runtime/core/src/child.rs` `a_child_gets_nothing_its_parent_lacks`; `plugins/resources/api/src/lib.rs`
  `an_agents_formula_declares_only_agents_skills_teams_and_goals_without_secrets`; `plugins/resources/core/src/module.rs`
  `a_namespace_is_an_expression_only_as_the_formula_uses_it`; `plugins/resources/host/src/overlay.rs`
  `a_boxs_owner_reads_as_the_user_but_never_secrets`
