# ADR-0076: A second folder with a taken project name gets its own name (`wisp`, `wisp-2`); a project stays keyed by its name

- Status: accepted
- Date: 2026-11-15
- Source: round 288: `wisp add /tmp/b/wisp` after `/tmp/a/wisp` replaced the first project, so the user lost it.
- Builds on ADR-0026 (a resource's name is its stable id, in files too). No stored shape changes; no migration.

## Context

- A project is a `project` resource of wisp.runtime, keyed by its resource name (ADR-0026). `wisp add` named it
  after the folder's last part and sent `resources.apply`, and an apply of a name that exists is an *update*. So a
  second folder with the same last part (`/a/wisp`, `/b/wisp`) moved the first project to the second folder.
- The shell's Run on (`packages/shell/src/frame/run-on.ts`) had the same bug on the other computer: it read
  `resources.export` as `{ projects }` (the server answers with `Resource[]`), so it knew none of that computer's
  projects and applied the checkout under the bare name, replacing a same-named project there.
- Projects' Add (`project-names.ts` `nameFor`) already named a second one `wisp-2`.
- v1 gave each project a random id, so two folders could share a name; it never had this bug.
- Re-keying projects by path would change the stored shape, every `project_for` caller, and the files: a spec's
  name is what a user writes (`projects: { wisp: { path: … } }`) and what other resources refer to.

## Decision

1. **A project stays keyed by its name.** Stored rows keep their names, so nothing migrates and existing projects
   keep working.
2. **Every path that adds a folder as a project picks a free name** instead of applying a taken one:
   `wisp_runtime_api::paths::project_name_for(path, projects)` (`wisp add`), `nameFor` (Projects' Add, Run on). The
   base is the folder's last part (`.git` off). Then `-2`, `-3` and so on, skipping every name that's taken, even one
   taken by hand for another folder. `wisp add` lower-cases the base and turns spaces into `-`, as it did before.
   Projects' Add keeps its own rule (case kept, other characters as `-`).
3. **The same folder again is the same project.** Any spelling of it counts (`same_folder`: trailing `/`, `/.`, case
   on Windows, and macOS's `/private/tmp` as `/tmp`, which a bare `wisp add` reads from its own folder). Nothing is written, and `wisp add` says `already a project: <name>`.
4. **In files, the second one is just `wisp-2`.** Export writes the stored names, and apply round-trips them. In a
   spec a name means one project: applying `wisp` with another path *moves* that project, as ADR-0026 says. To
   declare two same-named folders, give them two names (`wisp`, `wisp-2`, or any names).
5. The UI tells same-named projects apart by their folder (`packages/shell/src/frame/project-labels.ts`). That is
   display only; it never changes a name.

## Edge cases and their tests

- `/a/wisp` then `/b/wisp` are two projects, `wisp` and `wisp-2`, and `wisp add /a/wisp/.` changes nothing:
  `crates/wisp/tests/serve_cli.rs` `wisp_add_of_a_second_folder_with_the_same_name_keeps_both_projects`.
- `-3` after `-2`; a `wisp-2` taken by hand is skipped; a trailing `/.` is the same folder; `My App` gives `my-app`;
  `C:\work\Api\` gives `api`; `/` gives `project`: `plugins/runtime/api/tests/paths.rs`
  `a_second_folder_with_a_taken_name_gets_its_own_name_and_the_same_folder_stays_one_project`.
- Each folder's task finds its own project (`project_for`): `plugins/runtime/host/src/kinds/project.rs`
  `two_same_named_folders_are_two_projects_and_each_holds_its_own_tasks`.
- Run on: the other computer's `web` at another folder is kept, and the checkout joins as `web-2`:
  `packages/shell/test/run-on.test.ts` `…_r288_run_on_same_name`.

## Consequences

- No new command, wire message or migration. A project lost to the bug before this fix is not brought back: add its
  folder again.
- `resources.apply` still updates whatever project has the name it's given. That is right for wisp-as-code. An agent
  or script that adds a folder through `resources.apply` directly picks its own name. A server-side
  `runtime.projectAdd` that names it there would cover every client. It is not built: today's three add paths are
  covered.
- Run on names the checkout after the project it ran from (`web`, or `web-2` there when `web` is taken), not
  after the checkout's folder, so one repo keeps one name across computers where it can.
- Other symlinks to one folder are still two spellings: `same_folder` reads no file system (sans-IO).
- Two clients adding two same-named folders at the same moment can still choose the same free name. The later
  apply wins. That needs two adds within one round trip, so it is left open.
