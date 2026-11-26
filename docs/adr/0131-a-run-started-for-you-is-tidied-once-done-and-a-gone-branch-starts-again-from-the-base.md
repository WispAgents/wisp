# ADR-0131: a run started for you is tidied once it's done, and a task whose branch is gone goes on from the base

- Status: accepted
- Date: 2026-11-26
- Supersedes: ADR-0108's tidy trigger (its "On `turn.ended`" step). The rest of ADR-0108 stands: one kept folder per
  recurring key, `tidy` on every crew, trigger and schedule run, the strict removal and `BranchMode::IfMerged`, the
  CLI's own worktree default.
- Supersedes: ADR-0053's refusal of a turn whose worktree's branch is gone (audit F orchestration F2's message
  stays, for `worktrees.ensure` and an extra folder).
- Source: the owner, 2026-11-26, after the crew check-in "Daily agent pain check-in" refused its follow-up with "This
  task's branch wisp/moris-check-in is gone: it was merged (or deleted)". Nothing had been merged.

## Context

- ADR-0108 ported v1's `tidy()` on a crew job ending: on `turn.ended`, a `tidy` worktree whose task was idle (not
  running, not waiting on `ask_user`, nothing queued) went, its branch too when nothing was on it (`IfMerged`).
- v1 assumed a crew job asks nobody. A check-in can: its answer ended with a question card ("Turn which ideas into
  Linear Todo tickets?"). The card is part of the answer, not a paused `ask_user`, so the task landed in Review (For
  later), not Waiting. To the tidy it was finished.
- The run had changed nothing, so its branch pointed at a commit already on main. `IfMerged` counted it merged and
  deleted it with the folder.
- The user picked an option on the card. `bring_back` found no folder and no branch and refused the turn with the
  message written for a task whose PR merged. It was wrong twice: nothing was merged, and the task wasn't done.

## Decision

1. **Tidy once the task is done, never on a turn ending alone.** `archived::to_tidy` requires `archived` or
   `state == Done`: Mark done, archiving, or a quiet turn that runtime lands Done (`land.rs`: nothing to report, as a
   check-in with no news). A turn that lands in Review (an answer, a question, For later) keeps its folder.
   `wisp.worktrees` no longer listens to `turn.ended` for this. The tidy runs where archiving's removal already runs,
   on `session.changed` (`archived::on_changed`, coalesced per burst). A `tidy` worktree that's done is settled
   there, with the same guards as before: strict removal, kept, shipping, a /btw answering, another working task in
   the folder (`in_use`), an extra folder of a working task (`extra_in_use`), a claim since the look.
2. **A follow-up whose branch is gone goes on.** `archived::bring_back` calls `Engine::restore(wt, fresh: true)`.
   When the folder and its branch are both gone, the branch is made again at the base as it is now: `base_ref`
   (`origin/main`), or `base` (`main`) when `base_ref` is a sha (no remote), else the sha it branched from. The folder
   comes back on it, with its ignored files copied, as any restore does. Its work, if any, was merged or tidied only
   when nothing was lost, so it's on the base. The run then starts as usual.
3. **The turn says so.** `bring_back` returns `Ok(true)` when it made the branch again. The plugin keeps the line
   for that `(task, turn)` and posts it on the turn's `turn.started` (or `turn.held`) through `runtime.note` (tone
   info): "This task's worktree was deleted, so it goes on in a fresh one from main (branch wisp/x)." The capability
   `runtime:note` is added to `wisp.worktrees`. A side run (a /btw) has no turn and gets no line.
4. `Engine::ensure` (`worktrees.ensure`) and an extra folder (`bring_back_extra`) keep refusing a gone branch with the
   plain message: they aren't a task going on in its own folder.

## Consequences

- A crew check-in that asked something keeps its folder until it's marked done or archived. The cap and the hourly
  sweep (ADR-0108, `worktrees.max`) still bound the folders; a recurring key still reuses its one folder.
- A task is never refused for a branch that's gone; it never runs in the main checkout either.
- A branch deleted on purpose with unmerged work (`git branch -D` by hand) comes back from the base: that work is not
  in the fresh folder, and the line says the worktree was deleted.
- No wire shape changes: `Worktree.tidy`'s doc changes, `bring_back` returns `bool`, `Engine::restore` is new.

## Places it reaches (ADR-0040)

- Inside wisp: the task's turn shows the line in its activity.
- From the harness: the same line is a step of the turn that `runtime_turns` / `runtime.activity` return to an
  agent; the follow-up works the same from MCP (`runtime_followUp`) and the CLI (`wisp reply`).

## Edge cases and tests

- A done tidy run that changed nothing: folder and branch go (`tests/recurring.rs`
  `a_run_started_for_you_that_changed_nothing_…`); a Review turn doesn't tidy, the same one archived does.
- A follow-up after the branch went, with main moved on: made again at main's new tip, `Ok(true)`, a second turn
  finds it there and says nothing (`a_follow_up_after_its_branch_went_goes_on_in_a_fresh_worktree_from_the_base`).
- `worktrees.ensure` still says "branch … is gone … on main now" (`tests/flows.rs`).
- End to end: an archived task whose branch was deleted runs its follow-up in the remade worktree and the turn's
  activity has the notice (`crates/wisp/tests/serve_worktree_back.rs`
  `a_task_whose_branch_is_gone_goes_on_in_a_fresh_worktree_and_says_so`).
