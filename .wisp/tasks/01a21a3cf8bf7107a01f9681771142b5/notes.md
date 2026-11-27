# Notes

<!-- wisp keeps this file: the task's Notes (rev 7). Edit them in wisp, `wisp notes 01a21a3cf8bf7107a01f9681771142b5` or with wisp_notes. -->

## Goal
After Save to repo, offer Push so a task's saved .wisp/ files reach the remote instead of staying local. PR #996.

## Constraints
- Never touch the user's checkout, branch or index when pushing from the main checkout
- main is protected: open a PR, don't push to main
- Merge origin/main into the branch (no rebase); don't push, wisp pushes

## Decisions
- Owner picked the PR route for tasks saved into the main checkout: commit built in a scratch GIT_INDEX_FILE on origin/<base>, force-push to wisp/history-<short id>, gh pr create, then remove the local copies (delete untracked, checkout HEAD for tracked) so the next pull doesn't fail
- Task with a live worktree: git add + commit --only those files, push HEAD to the task branch
- No Wisp-Task trailer on the history commit (it marks code commits)
- ADR-0137 records it

## Tried and failed
- Leaving the local copies untracked after pushing elsewhere makes git pull fail ('untracked files would be overwritten'); staging them still breaks merge and rebase pulls (tested in /tmp)
- Test servers fail to start unless every WISP_* env var is unset

## Where it stands
Done and merged with origin/main three times; each time the only conflict was CHANGELOG.md Unreleased (both sides kept: Added from this task, Changed/Fixed from main). Checks pass on the last merge: npm run check, typecheck, history vitest, clippy, the serve_history push e2e test. Not covered: an e2e test for the worktree-branch path.

## Key files
- plugins/history/host/src/push.rs
- plugins/history/api/src/lib.rs (PushArgs, Pushed, history.push)
- plugins/history/ui/src/SavedLine.tsx, save.ts, later.ts, OfferFoot.tsx
- crates/wisp/tests/serve_history.rs (a_saved_task_is_pushed_as_a_pr_without_touching_the_checkout)
- docs/adr/0137-a-saved-tasks-history-is-pushed-as-a-pr-without-touching-the-checkout.md
