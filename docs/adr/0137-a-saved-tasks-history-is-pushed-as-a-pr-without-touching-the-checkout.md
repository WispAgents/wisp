# ADR-0137: a saved task's history is pushed as a PR without touching the checkout

- Status: accepted
- Date: 2026-11-27
- Source: owner, 2026-11-27 ("when a task is done and I click Save to repo, add an option to push the code so it
  doesn't just stay local"); the PR route picked over a commit on the current branch.
- Changes: ADR-0033's Save to repo (additive: one command, `history.push`, on `wisp-history-api`).

## Context

Save to repo (`history.track` / `history.save`) writes a task's spec, report and transcript into `.wisp/` and stops:
nothing commits them. For a task whose worktree still holds unshipped work that's fine, Ship it carries them. But a
task marked done after its PR merged (the common case for `track: ask`) saves into the **main checkout**
(`config::root_of_task`), where the files sit untracked until the user commits them by hand, on a `main` that is
often protected.

Pushing them from a side branch while leaving the local copies breaks the user's next pull: git refuses a merge that
would overwrite untracked files even when they're identical ("untracked working tree files would be overwritten by
merge"). Staging the copies instead lets a fast-forward through but still stops a merge pull and any rebase pull
("Your index contains uncommitted changes"). Both checked by hand on git 2.4x.

## Decision

1. `history.push {id}` (an MCP tool like every command, ADR-0040) pushes what the task's last save wrote
   (`ext["wisp.history"].tracked`); a task not saved is refused with "Save to repo first".
2. **Saved into the task's live worktree** (`ext["wisp.worktrees"].path` contains the saved repo): `git add` and
   `git commit --only` those paths (anything else staged stays staged), then push `HEAD` to the task's branch.
3. **Saved anywhere else (the main checkout):** the commit is built in a scratch index (`GIT_INDEX_FILE` in the temp
   dir): `read-tree` of `origin/<base>` (from `origin/HEAD`, else `main`/`master`, fetched first), `hash-object -w`
   each file (through the repo's filters, so an LFS store's transcripts go in as pointers), `update-index`,
   `write-tree`, `commit-tree`. It's force-pushed to `wisp/history-<short id>` (one branch per task: a later save
   replaces it) and `gh pr create` opens the PR (or `gh pr list` finds the open one). The user's checkout, branch, HEAD
   and index are never touched. The commit carries no `Wisp-Task:` trailer: that marks the task's own code commits.
4. Then the local copies are taken out: deleted when untracked, `git checkout HEAD --` when tracked, so the next pull
   brings them back. A tree equal to the base pushes nothing and opens no PR (`branch: ""`).
5. The task's slot records `pushed: {branch, commit, pr}`. The UI shows **Push** on the saved line under the answer
   and **Push to repo** in ⋯ for a saved task; pushed, the line says where with **Open PR**.

## Consequences

- Until the PR merges and the user pulls, Repo history on that computer doesn't list the task (its files are out of
  the checkout). That's the price of a pull that never stops; the PR is the one place the files are.
- Needs `origin` and, for the PR, `gh`; without `gh` the branch is still pushed and the answer has no `pr`.
- The `notes` store's transcripts (git notes) aren't pushed by this; its `.wisp/tasks/` files are.
