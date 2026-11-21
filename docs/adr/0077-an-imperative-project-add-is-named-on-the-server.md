# ADR-0077: An imperative project add is named on the server (`runtime.projectAdd`); `resources.apply` stays declarative

- Status: accepted
- Date: 2026-11-15
- Source: round 290. Builds on ADR-0076 (its naming rule) and ADR-0026 (a spec's name is its id). Supersedes
  ADR-0076's Consequences lines "No new command" and "two clients adding two same-named folders at the same moment can
  still choose the same free name".

## Context

- ADR-0076 put its naming rule (same folder = the project it is; else the base, `-2`, `-3`) in two clients: `wisp add`
  and the shell's Run on. Projects' Add had its own copy (`nameFor`, case kept). Every other client (an agent through
  MCP, the phone, a script) still sent `resources.apply` with a name it chose, and an apply of a taken name is an
  update: it moved the other folder's project.
- Each client read the projects, chose a free name and then applied it: two round trips. Two adds within one could
  choose one name, and the later replaced the earlier.

## Decision

1. **`runtime.projectAdd { path, name? } → { name, path, created }`** is the imperative add. wisp.runtime spells the
   path where it enters (`~`, canonical, 8.3 names spelled out: ADR-0071, `kinds::project_path`), refuses one that
   isn't a full path (a relative path is the server's own folder; a URL is cloned first, `git.clone`), and applies
   ADR-0076's rule against the stored projects **inside one store transaction** (`StoredKind::put_with`, additive in
   `wisp-resources-api`): the read and the write are on the store's one writer, so two adds never both find a name
   free. The same folder again writes nothing and answers `created: false` with its name. A `name` given is the base:
   taken, it gets `-2`.
2. **Every client adds through it**: `wisp add`, Projects' Add (and Other folder…, Browse, Found on this machine; the
   phone uses the same pane), Run on (on the other computer, asking for the project's own name), and agents: its MCP
   tool `runtime_projectAdd` is generated from the catalogue (ADR-0040). The clients' naming is gone; Projects keeps a
   guess (`addedName`) only for the optimistic row, replaced by the server's answer.
3. **Permissions** (default deny): `runtime:projectAdd`, reached by `user:full`, `agent:session` and `apikey`
   (`runtime:*`) and `box`; refused to `agent:capped` ("add a folder to the user’s projects": a task a channel started
   below Full auto doesn't change the user's projects).
4. **`resources.apply` keeps its meaning**: a spec's name is its id (ADR-0026), so applying `wisp` with another path
   moves that project. Files, formulas and wisp-as-code go through it; a client that adds a folder goes through
   `runtime.projectAdd`.

## Edge cases and their tests

- Two adds at once of `/a/wisp` and `/b/wisp` are `wisp` and `wisp-2`: `plugins/runtime/host/tests/project_add.rs`
  `two_concurrent_adds_of_same_named_folders_give_wisp_and_wisp_2`.
- The same folder twice, as `/x`, `/x/.`, `/x/` (and with another name asked), is one project, `created: false`:
  `the_same_folder_twice_gives_one_project`.
- A caller's name that's taken gets `-2`; a name is trimmed: `a_caller_given_name_thats_taken_gets_2`.
- A relative path, an empty one and a URL are refused; a caller whose set doesn't name it is refused:
  `a_relative_path_is_refused_and_so_is_a_caller_without_the_permission`.
- A capped task is refused in the capped wording; a task's agent at Full auto adds:
  `a_capped_task_is_refused_and_an_uncapped_agent_adds`.
- A name with `/`, `\`, `:` or a control character is refused, never stored as an id (`my.app` is fine):
  `a_name_with_separators_or_control_characters_is_refused`.
- `/repo/.` typed in Projects is the project `/repo` already (no row named `.`): `packages/shell/test/projects.test.ts`
  `…_adr0077_guess`.
- An agent adds through its MCP tool: `crates/wisp/tests/serve_mcp_tools.rs`
  `an_agent_adds_a_project_through_its_mcp_tool_and_the_server_names_it`; `wisp add` keeps ADR-0076's test
  (`serve_cli.rs` `wisp_add_of_a_second_folder_with_the_same_name_keeps_both_projects`).
- Run on asks that computer to add its checkout, never applies a name: `packages/shell/test/run-on.test.ts`
  `…_r288_run_on_same_name`; the dev mock names as the server does: `apps/web/test/mock-commands.test.ts`.

## Consequences

- One rule, one place: Projects' Add now names as `wisp add` does (lower case, spaces as `-`), where it kept case.
- A project added through `runtime.projectAdd` has no `resources_applied` baseline; a later plan shows no drift for it
  (as for any project made in the app).
- Run on against a computer whose wisp predates the command adds nothing there (the call fails quietly, as before);
  the next launch clones or finds the checkout again.
- Agents and API keys (`runtime:*`) may now add a project, which they couldn't through `resources.apply`
  (`resources:*` isn't theirs): on purpose, so an agent can set up the folder it was asked to work in.
- `wisp add` against a wisp that predates the command (`--on` an older peer) fails with an unknown command instead of
  applying a name.
