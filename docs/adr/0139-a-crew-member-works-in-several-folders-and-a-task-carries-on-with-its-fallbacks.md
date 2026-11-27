# ADR-0139: a crew member works in several folders, and a task carries on with its fallbacks at a usage limit

- Status: accepted
- Date: 2026-11-27
- Source: owner, 2026-11-27: "One crew should have control to many folders set by user, also they should have
  fallback models."
- Changes: additive fields `Member.dirs`, `Member.fallbacks` (`MemberBody`, `CrewSpec`, the `wisp_crew_add`/`_edit`
  tools), `StartArgs.fallbacks`, `Session.fallbacks`. No new wire message, no migration (members are JSON rows;
  sessions read missing fields as empty).

## Context

A member had one folder (`cwd`), and every job ran there. A member who looks after an app and its API, or several
repos, needed one copy per folder. When a job's run hit its plan's usage limit, the task stopped with the limit card
(or waited for the reset). Auto's pipeline moves on to another harness, but Auto is off by default (owner-list #37)
and picks the harness itself. A member had no way to say "then use Codex, then Sonnet".

The runtime already runs a task with extra folders (`StartArgs.dirs`, which Claude Code and Codex get as
`--add-dir`; the other harnesses don't take them), so only the member needed to keep a list.

## Decision

1. **Folders.** A member keeps `cwd` (their own folder, where every job starts and gets its worktree) and `dirs`, up
   to 10 more absolute folders. They are checked on this computer when the member runs here, never `cwd` twice, and
   cleared with `cwd`. Every job gets them as the task's `dirs`. A sandbox job gets none, because a sandbox has its
   own copy of `cwd` only. The job's crew note lists them too ("Your folders"), so an agent on a harness without
   `--add-dir` still knows where it may work. A capped task sees a member when its folder is inside any of the
   member's folders.
2. **Fallbacks are the runtime's, per task.** `runtime.create` takes `fallbacks: [RunPick]`, and the session keeps
   them. When a run lands at its usage limit (`turn.limit`, the router's classify) or its model is at capacity, the
   runtime picks the first fallback whose harness is here and that no run of this task has stopped on yet. It lands
   the task quietly with the hero saying where it carries on, and sends `runtime.followUp` with that `run` (by
   `fallback`) in the same conversation. It moves at most once per fallback, so a list that is all at its limit
   ends with the ordinary card and never loops. A fallback goes before Wait for the reset; with none left, the wait
   applies as before.
3. **The crew only hands its list on.** A member keeps `fallbacks: [{agent, model?, effort?}]`, up to 5: a harness
   wisp runs (never Auto) or '' for theirs. Every job gets them as `runtime.create`'s `fallbacks`. Putting the move in
   the runtime and not in a crew listener means it can never race the runtime's own Wait for the reset, and any
   task (a formula, the CLI, MCP) can declare fallbacks later through the same field.
4. **Never another account.** Fallbacks are harnesses and models the user named. wisp still never moves a task to
   another account of the same provider by itself (design/accounts.md).

## Where it reaches (ADR-0040)

The crew editor (More folders and Fallbacks on its bar), `wisp crew create|update --dirs … --fallbacks …` (the CLI's
generated flags), the `wisp_crew_add`/`wisp_crew_edit` MCP tools, and `crew:` in `wisp.yml` (`dirs`, `fallbacks`).

## Consequences

- A fallback's model is set from the CLI, MCP or a formula. The editor picks harnesses in order (each on its default
  model) and keeps a model set elsewhere, as it does for the member's own model.
- Extra folders on another machine are that machine's paths. They are not checked here.
- Tests: `wisp-runtime-core` `limit::tests` (order, never loops, capacity, harness gone), `wisp-runtime` lifecycle
  `a_run_at_its_usage_limit_carries_on_with_the_tasks_fallback`, `wisp-crew-core` `tests/folders_fallbacks.rs`,
  `wisp-crew` `a_job_gets_the_members_folders_and_fallbacks`.
