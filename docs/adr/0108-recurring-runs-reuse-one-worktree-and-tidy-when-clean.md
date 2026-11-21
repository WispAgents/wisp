# ADR-0108: recurring runs reuse one worktree, runs started for you leave none when clean, and the CLI keeps its own worktree default

- Status: accepted
- Date: 2026-11-19
- Source: orchestration audit C (`docs/qa/audit-C-orchestration.md`) C3, C4 and C8, and the owner's delegate's decision
  for C3: "recurring runs must never grow worktrees without bound".

## Context

- **C3.** The fix for audit B #7 gave every crew job, check-in and trigger run its own worktree, as v1 gave every launch
  one. With a member's cron and a trigger both set to every minute, one project went from 25 to 65 worktrees in 40
  minutes, past `worktrees.max` 40. Each run had its own branch (`wisp/ada-check-in-<x>`, `wisp/uitrig-<x>`). The
  fake's uncommitted edit kept each folder from the cleanup.
- **v1** (`server/services/launch.js` `wantTree`) also made one worktree per launch. It also ran `tidy()`
  (`services/done-folders.js`) when a crew job ended: whatever nothing would be lost from went, and nobody was asked.
  v2 had no such step for a run started for you.
- **C4.** v1's launcher toggle wrote `defaults.worktree` for every later launch, the CLI's included. v2's Where ›
  In place writes the same shared switch (`worktrees.worktree`). Five CLI launches after one In place pick ran in the
  user's checkout, and the menu didn't say the pick was kept.
- **C8.** A rerun at an older commit is checked out detached in `<home>/worktrees/rerun-<sha>-<x>`. The rerun's own
  worktree was then branched from the base, not from that commit. The detached folder stayed, and Settings ›
  Worktrees listed it as "No task".

## Decision

1. **Two additive fields on `runtime.create`'s `StartArgs`**, passed on to `worktrees.create`:
   - `worktreeKey`: a recurring run's key.
   - `worktreeTidy`: the run was started for you.

   The same two are added to `wisp_worktrees_api::CreateArgs` (`reuse`, `tidy`). `Worktree` gets `tidy`, so the
   task's slot remembers it.
2. **A recurring run reuses one worktree per key and project.** The folder is `<home>/worktrees/<project>-<key>` and
   the branch is `wisp/<key>`:
   - A member's check-ins use `<member>-check-in`.
   - A trigger's or schedule's runs use `trigger-<name>` or `schedule-<name>`.

   The kept folder is reused when it is this repo's worktree. When it is missing, it is made again on its branch, or a
   new branch is made from the base. A run whose kept folder another working task is in gets a fresh folder of its
   own. A run you start (`wisp run`, a crew job you gave) gets a worktree of its own, as before.
3. **A run started for you leaves no worktree when it changed nothing.** This covers every crew job, check-in, and
   trigger or schedule run. It is v1's `tidy` on a crew job ending:
   - On `turn.ended`, wisp.worktrees looks at a `tidy` worktree whose task is idle, has nothing queued, and isn't kept
     or shipping.
   - It removes the folder strictly: anything only the folder holds keeps it.
   - The branch goes too when nothing is on it.
   - Nothing is removed while another working task is in the folder.
4. **In place stays sticky, as in v1, and says so.** New task's Where menu says the pick is kept from now on and that
   the CLI keeps worktrees. The CLI and `wisp mcp` no longer read the shared switch: in a project they start a worktree
   unless `--no-worktree` is given or the mode is Plan.
5. **A rerun at an older commit** gets its worktree branched from that commit (`worktrees.create` `from`: the detached
   folder). Once the rerun runs in that worktree, the detached folder is removed.

No new wire message, kind or dependency. Each field is optional and defaults to the behaviour before this ADR.

## Edge cases (each has a test)

- Six check-ins give one folder on `wisp/ada-check-in`, so git knows one worktree. A check-in while that folder is in
  use gets a fresh one. A removed folder comes back on its branch with what was committed there:
  `a_members_check_ins_reuse_one_worktree_and_a_busy_one_gets_a_fresh_folder` (worktrees `tests/recurring.rs`).
- These cases are covered by
  `a_run_started_for_you_that_changed_nothing_leaves_no_worktree_and_one_with_work_stays`:
  - a trigger's clean run leaves no folder and no branch;
  - a job with an edit keeps its folder;
  - a running task, a task you started, and a folder another working task is in are left alone.
- Crew sets the key for check-ins only and `tidy` for every job:
  `a_check_in_reuses_the_members_worktree_and_every_job_tidies_its_own`.
- Triggers and schedules set both, and `wisp run` sets neither:
  `a_triggers_and_a_schedules_runs_reuse_one_worktree_and_yours_has_its_own`.
- The CLI's own default is a worktree, except in Plan mode:
  `a_cli_launch_in_a_project_gets_its_own_worktree_whatever_new_task_picked_audit_c4`.
- The Where menu says In place is kept and that the CLI keeps worktrees:
  `where_in_place_says_it_is_kept_for_every_new_task_and_the_cli_keeps_its_own_worktree_audit_c4`.
- A rerun at an older commit leaves no detached folder, and its branch starts at that commit:
  `a_rerun_at_an_older_commit_leaves_no_detached_folder_and_runs_at_that_commit` (serve_worktree_safety).

## Consequences

- A member's check-ins share one folder, so a check-in starts with whatever the last one left uncommitted. This is
  the owner's choice: one bounded folder rather than one per run. A crew member's notes and reports stay on its task.
- Overlapping recurring runs (a long run while the next fires) still get a folder each. Each goes when its run ends
  clean, so the count stays bounded by the runs in flight, not by time.
- `wisp "x"` in a project gets a worktree even when New task's last pick was In place. That pick no longer reaches
  the terminal.
