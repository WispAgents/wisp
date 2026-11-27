# Push after Save to repo

## when task is done and user click on save to repo, we need to add the option to push the code or something like that so it does not just stay local

Saved tasks now have a **Push** button, so their `.wisp/` files reach your remote instead of staying on your computer. It's built, tested, and I clicked through it in a real wisp in Chrome. Nothing is committed yet.

**What Push does:**
- **Task still has unshipped work in its worktree:** commits just the saved `.wisp/` files on the task's branch and pushes it, so its PR gets them.
- **Task already merged (files saved into your main checkout):** this is the PR route you picked. It builds the commit on top of `origin/main` without touching your checkout, branch or staged files, force-pushes it to `wisp/history-<id>`, and opens a PR with `gh`. Then it removes the local copies: left in place, your next `git pull` would stop on "untracked working tree files would be overwritten" (I reproduced that with plain git). They come back with that pull once the PR merges.
- **Where you find it:** **Push** sits next to "Saved to the repo: N files…"; once pushed, the line says "Pushed to wisp/history-…, pull request open" with **Open PR**. The ⋯ menu also gets **Push to repo** for any saved task. Agents get the same as the `history_push` tool.

**Tests and checks:**
- **New end-to-end test** (`crates/wisp/tests/serve_history.rs`, against a real `wisp serve` and a bare origin) passes. It checks:
  - Push refuses a task that hasn't been saved.
  - The branch lands on origin, built on main, with a PR.
  - Your HEAD, branch and index are unchanged, and an unrelated uncommitted file is left alone.
  - The local copies are gone, and `git pull --ff-only` works after the merge and brings them back.
- **UI unit tests** for the push state and the menu condition pass.
- `npm run typecheck`, `npm run check` (0 errors) and `cargo clippy -D warnings` all pass.
- **In Chrome** (desktop light, phone dark), the flow went Save → Push → Pushed with Open PR. On the phone the text cuts off with "…" instead of wrapping.

**Limits:**
- The task-worktree path isn't covered by the end-to-end test; only the main-checkout path is.
- Until the PR merges and you pull, that task doesn't show in Repo history on this computer, because its files are out of the checkout.
- Without `gh`, the branch is still pushed but no PR is opened.
- If the repo keeps transcripts in git notes (the `notes` store), those aren't pushed.
- The saved/pushed line only shows in the window that pressed Save, as before. After a reload, use ⋯ → **Push to repo**.

I also added a CHANGELOG entry under Unreleased → Added, updated two docs pages, and wrote ADR-0137 (Push adds a new command to the history plugin's API). The scratch server and Vite I used for the browser check are stopped.

**Push added to Save to repo**: After saving, Push commits the task's .wisp/ files: on its branch if its work isn't shipped, otherwise as a PR from wisp/history-<id> built on origin/main without touching your checkout.

## Files

- `plugins/history/api/src/lib.rs`
- `plugins/history/host/src/push.rs`
