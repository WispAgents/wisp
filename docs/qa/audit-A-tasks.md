# Audit A: tasks, code and daily use (wisp v2 @ 2013facf, 2026-11-18)

**Setup:** a scratch wisp, nothing real touched.
- `target/debug/wisp` (copied to /tmp/qaC/wisp), serving on **:4933**.
- `WISP_HOME=/tmp/qaC/wisphome`, `HOME=/tmp/qaC/home`, and the built UI (`vite build` to /tmp/qaC/web, served by wisp itself).
- Every harness points at wisp-testkit's fake agent (`WISP_*_BIN`); gh points at the fake gh (`WISP_GH_BIN`, state in /tmp/qaC/gh-state.json).
- Fake GitHub remote: `https://github.com/acme/app.git` is rewritten by `insteadOf` to the bare repo /tmp/qaC/remotes/app.git.
- Projects: `app` (with the remote), `plain` (git, no remote), `notes` (no git).
- From the first restart on: `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0`, so nothing goes to the wisp team.
- A `claude` wrapper replays a rich answer for prompts containing RICH (every block kind).
- Browsers: Chrome and WebKit (Playwright) at 1280 × 800. Phone screens at 390 × 844 in Chrome emulating the Android app (`window.wispAndroid` and a wispMobile UA, CDP touch).
- Scripts: /tmp/qa-keep/A/*.mjs. Shots: /tmp/qa-keep/shots-A3/ (210 files).
- **Feedback** = ms from pointerdown or keydown (in the page) to the first DOM change.

The worktree has uncommitted changes from another session (TaskView.tsx, shell.tsx, share host…). The build includes them.

## Counts

- **95 action rows** (some rows group a few related actions, ≈130 actions in all): 70 pass, 19 fail, 6 not checked or n/a. Most passes were run in both engines.
- **19 findings**: 1 critical, 7 major, 11 minor.

## Actions

C = Chrome, W = WebKit, P = phone (Chrome emulating Android at 390). "API" means the effect was checked with `runtime.list`, `runtime.turns`, git or the fake gh's state, not only on screen.

### Home and list

| Action | Result | How checked |
|---|---|---|
| New task (launcher Send), claude / RICH / gated | pass | C W; task created, navigates at once (fb 4–11 ms), runs, fake edits math.js |
| Open row (click) | pass | C W; URL → task |
| Row ⋯ / right-click menu (15 items, Task options ›, Copy ›) | pass | C |
| Pin / Unpin (row menu) | pass | C W; Pinned section, API `pinned` |
| Mark read / unread (row menu) | pass | C W; API `unread` toggles |
| Save for later + Undo (row menu) | pass | C W; For later section, API `forLater`, Undo restores |
| Save for later (hover) + Undo | pass | C |
| Archive + Undo (row menu) | pass | C W; API `archived` true → false |
| Mark done (hover) → worktree dialog → Keep | pass | C W; state done, folder kept |
| Mark done → Remove (ticked) | pass | C; folder removed, branch kept, "Removed 1 folder · 16 KB" (v1 parity) |
| Mark done Undo (hover toast) | pass | C; state back to review |
| Reopen (hover, Done section) | pass | C W |
| Rename (row) → Save | pass | C W; API title |
| Copy task ID / Copy resume command | pass | C (clipboard); W toast only |
| Delete task (row) → confirm → Delete | pass | C W; asks first, row gone, API gone (no Undo; v1 parity) |
| Open project in editor (row) | pass | C W; `plugin:opener|open_url vscode://file/<worktree>` |
| Fold / unfold sections | pass | C W; 11 → 1 → 11 rows |
| Hide / show the list (button and [) | pass | C W |
| Project switcher: only "plain", ⌘0 all, ⌘1 | pass | C W |
| ⌘K: find task, deep search ("tidy the readme" → Add fix) | pass | C W |
| ⌘K: Keyboard shortcuts, Dark / Light theme, Add project… | pass | C W (Add project… fb 121 ms once in W) |
| Multi-select on desktop | n/a | doesn't exist on desktop (phone only); v1 had none on desktop either |
| My Stuff, Make a pane / Pin pane / Publish / Launch app | not checked | no pane or app in the data (Make a pane runs a harness; the fake can't make a pane); Publish needs a grid |

### Keys (Home, picked row; task open)

| Key | Result | How checked |
|---|---|---|
| J / K select | pass | C W |
| Enter / O opens | pass | C W |
| E done (picked row) | pass, no Undo | C W (#13) |
| ⌫ / Delete (picked row) | **fail** | C W; deletes with no confirm (#4) |
| Enter after ⌫ | **fail** | C W (#11) |
| L / M / ⇧R / V on a picked row | **fail** | C W; nothing happens (#12) |
| ? shortcuts, Esc, [, N, ⌘K, ⌘P, ⌘0, ⌘1 | pass | C W |
| ⌫ on an open task | **fail** | C W (#4) |

### Task page

| Action | Result | How checked |
|---|---|---|
| Reply: Send / Enter | pass | C; shown at once, new turn, the agent got the text (fake record) |
| Stop a running turn (dialog → Stop) | pass | C; "Stop Claude Code?", turn stopped |
| Rename by clicking the title | **fail** | C W (#18) |
| Head ⋯ Rename | pass | C |
| Transcript (menu) | pass | C |
| Copy task ID (head) | pass | C |
| Open in Claude Code (T) | pass | C; terminal tab running `claude` |
| Agent notes toggle | pass | C |
| Restore to before this turn | **fail** | C W, API (#1) |
| Thumbs up / down + why | not checked | hidden with WISP_FEEDBACK=0; would post to the wisp team |
| Fork… (head) → Fork | **fail** | C W, API (#2) |
| Fork from this turn / Fork… (row) | **fail** | C, API (#2) |
| Rerun… → Rerun | **fail** | C W, API (#3) |
| Side discussion… → send | **fail** | C W, fake record (#6) |
| Set goal… → Set goal | pass | C; goal band, the judges' follow-ups ran |
| Drop goal | pass, stale question | C (#15) |
| Add to crew… → Add | pass | C; `crew.list` has qabot, "qabot joined your crew" |
| Save as recipe… → Create | **fail** | C W (#7) |
| Share… | blocked by design | C; Share disabled: "Sign in to a wisp Grid first (Settings → Cloud)" |
| Head Pin / Unpin, Mark unread, Save for later | pass | C W |
| Head Archive | pass, no Undo | C ×2 (#13) |
| Head Done / Reopen | pass | C W |
| Head ⋯ Delete task | **fail** | C W (#4) |
| Comment on a block → Comment → Send comments | pass | C; follow-up `<comments>…` |
| Question option "1 Add tests" | pass | C; follow-up "Add tests for add()" |
| Actions block Run (`ls -la`) | pass | C; `runtime.exec`, output shown |
| Actions block Send | pass | C; follow-up "Write docs for add()" |
| Next chip | pass | C; drafts "Add subtract()" |
| Tests Re-run | pass | C; `runtime.exec` |
| Code block Copy / Save to My Stuff | pass | C; clipboard, `stuff.save` |
| Files "Open in editor", code and test file links | **fail** | C (#10) |

### Side panel and ship

| Action | Result | How checked |
|---|---|---|
| Changes: open (chip, D) | pass | C W |
| Stage / Unstage a file, Stage a hunk | pass | C; `git diff --cached` |
| Edit here + Save (⌘S) | pass | C (on a finished task); refused during the goal question (#15) |
| File ⋯ Revert file | pass | C; file back to HEAD, status clean |
| File ⋯ Copy path | pass | C |
| Commit | pass | C; new commit "Commit me", status clean |
| Head Changes chip count | **fail** | C W (#9) |
| Shell: type `echo hello-$((40+2))` | pass | C W; "hello-42" |
| Shell: End shell | pass | C |
| Preview: Run with empty "Runs with" | **fail** | C W (#16) |
| Thread: add Goal note | pass | C |
| Place ⋯ (Copy path / Open in editor) | **fail** | C W (#19) |
| Ship it… → Commit & open PR | pass | C; fake gh PR #1 and #2, branch pushed to the fake remote |
| PR strip Check now | pass | C; `ship.refresh` |
| PR strip Merge → confirm → Merge PR | pass | C; PR MERGED, origin main 82b19b7 → 6329bff, worktree removed; toast cut off (#16) |
| PR pane Merge | **fail** | C (#5) |
| Ship it… after merge | pass | C; "Nothing to ship yet" |

### Desktop

| Action | Result | How checked |
|---|---|---|
| Window drag: Home top / dblclick | pass | C W; `start_drag`, `zoom_window` |
| Window drag: task head edges | pass | C W |
| Window drag: task head middle (title) | **fail** | C W (#18) |
| Native menus (About, Edit, View…) | not checked | OS menus, no Tauri window here |

### My Stuff

| Action | Result | How checked |
|---|---|---|
| Add files (upload 2) | pass | C; `stuff.list` 1 → 3 |
| Card menu | pass | C |
| Open CSV viewer | pass | C W |
| Star | pass | C; `starred` |
| Edit details → Save | pass | C; title "QA notes" on the card |
| Use in a new task | pass | C; `@[fruit](stuff:…)` in the launcher |
| Remove + Undo | pass | C W; hidden at once, back after Undo |
| Remove (no Undo) | pass | C; items 3 → 2 |

### Phone (390, Chrome emulating Android)

| Action | Result | How checked |
|---|---|---|
| Phone shell (bar, FAB, bottom nav Tasks · Crew · Stuff) | pass | P |
| Swipe right (Mark done) / left (Save for later) | **fail** | P ×4 (#8) |
| Long-press (contextmenu) → row sheet with Select first | pass | P |
| Bar ⋯ → Select → pick 2 → Archive / Save for later / Mark done → Undo | pass | P; two commands each, "Archived 2 tasks · Undo", Undo restores |
| Pull to refresh | not checked | the CDP pull hung my script; not retried |
| WebKit phone gestures | not checked | no touch injection in WebKit |

## Findings

### 1. [critical] "Restore to before this turn" resets the project's main checkout, not the task's worktree
- Engine: Chrome (UI and API) and WebKit (UI), at 1280.
- Steps:
  1. Have a task that ran in its own worktree and changed `math.js` ("Subtract" or "Victim E").
  2. Put an uncommitted change and an untracked file in the project's main checkout (`repos/app/README.md` and `untracked.txt`).
  3. On the task, press the turn's ↺ "Restore to before this turn".
- Expected: the task's worktree goes back to before the turn (`math.js` back to `a - b`), and nothing outside it is touched.
- Actual:
  - The toast says "Worktree restored to before this turn".
  - The worktree's `math.js` still reads `a + b`.
  - In the **main checkout**, the README change was reverted and `untracked.txt` was **deleted**. There's no confirm.
  - Calling `git.restoreTurn` by hand does the same (`{"restored":true}`, main checkout's `notes.txt` change gone).
- Likely cause: `plugins/git/host/src/write.rs:251-262` runs `ls-files --others` / `remove_file` / `read-tree --reset -u` / `reset --mixed` in `self.info.root` (the main repo, `service.rs:110`), not `self.dir` (the worktree).
- Shots: chromium-Restore_to_before_this_turn.png, webkit-Restore_to_before_this_turn.png

### 2. [major] Fork shares the original task's folder: no copy and no branch of its own
- Engine: Chrome (head, row and "Fork from this turn") and WebKit (head); checked with the API.
- Steps: a finished worktree task → ⋯ → Fork… → write a prompt → Fork.
- Expected: as the dialog says, "A new task with the whole conversation and a copy of its files, on its own branch".
- Actual:
  - The fork's `cwd` is the original task's worktree: "Forked" `…fd2d97` and `…095366` sit in `app-…f631` and `app-…c426`.
  - `branch` is undefined.
  - Forks of a task in the main checkout run there too (`a31367`, `df79d1`).
  - The fork's agent edits the original task's files.
- The conversation copy itself is right: turn 1 and the fork prompt.
- Shots: chromium-Fork_head_Fork.png, webkit-Fork_head_Fork.png, chromium-Fork_row_menu_Fork.png

### 3. [major] Rerun of a worktree task runs in the project's main checkout
- Engine: Chrome and WebKit.
- Steps: a worktree task ("Subtract", "Victim E renamed") → ⋯ → Rerun… → Rerun.
- Expected: a new task, isolated like the original. Home's launch in the same project gets its own worktree ("This computer · Worktree").
- Actual:
  - The dialog says "Starts a new task in /private/tmp/qaC/repos/app".
  - The new task's `cwd` is the main checkout, with no branch.
  - The fake's edit landed in `repos/app/math.js` (git status ` M math.js`, seen 2×). That then blocked "Local main was not updated" after the merge (#16).
- Shots: chromium-Rerun_Rerun.png, webkit-Rerun_Rerun.png

### 4. [major] Delete from the head ⋯, ⌫ or Delete deletes at once: no confirm, no Undo
- Engine: Chrome and WebKit.
- Steps:
  - (a) open a task → ⋯ → Delete task;
  - (b) open a task, press ⌫ or Delete;
  - (c) Home, J to pick a row, ⌫.
- Expected: the row menu's "Delete this task from wisp? The agent's own history is kept. · Cancel · Delete", as v1 did for every path (`web/js/features/actions.js:345`, `keys.js:109`).
- Actual: `runtime.delete` goes at once. The task is gone (API), with no dialog and no toast. From a task page it then opens another task. Seen 6 times (Victims H–M).
- Shots: chromium-headdel-menu.png, chromium-headdel-Backspace.png, webkit-headdel-Delete.png

### 5. [major] The PR pane's Merge merges and removes the worktree with no confirm
- Engine: Chrome. The WebKit retry couldn't make a second PR, because its change was already on origin.
- Steps: a task with an open PR → side panel PR tab → Merge.
- Expected: the confirm that the PR strip and Ship ▾ show: "Squash-merge PR #1? Then wisp fast-forwards your local base branch and removes the worktree." v1 asked too (`ship.js:391`).
- Actual: `ship.merge` goes at once. PR #2 is MERGED and the worktree `app-…12e7` is deleted. There's no dialog and no toast. (`plugins/shell-task/ui/src/side/PrPane.tsx:138` calls it directly.)
- Shots: chromium-pr-pane.png, chromium-merged.png

### 6. [major] "Side discussion…" sends "/side …" into the main conversation
- Engine: Chrome and WebKit.
- Steps: ⋯ → Task options → Side discussion… (the box fills with "/side ") → type "is this the right approach?" → Send.
- Expected: a side discussion on a page of its own (v1 `composer.js:639`, `side.js:145`).
- Actual: a normal follow-up on the main task (turns 1 → 2). The agent got "/side is this the right approach?" as its prompt (fake record).
- Shots: chromium-Side_discussion_send.png

### 7. [major] Save as recipe… fails with the name it fills in itself
- Engine: Chrome ("Add fix") and WebKit ("Victim E renamed").
- Steps: ⋯ → Task options → Save as recipe… → Create recipe.
- Expected: the recipe is saved. Or, if names must be slugs, the field says so and fills in a slug.
- Actual:
  - Name is pre-filled with the task title.
  - Create sends `automations.task.set` and gets a raw error toast: `a task's name is letters, digits, - _ . (not "Victim E renamed")`. The dialog stays open.
  - Typing "victim-e" works: it shows under Home → Recipes.
  - The same applies to Recipes › New with any name containing a space.
- Shots: webkit-recipe-prefilled.png, chromium-Save_as_recipe_Create_recipe.png

### 8. [major] Phone: swiping a row arms and flies it off, but the action never runs
- Engine: Chrome emulating the Android app at 390 (`window.wispAndroid`, CDP touch); 4 of 4 tries.
- Steps: Tasks → swipe a row right (Mark done) or left (Save for later).
- Expected: the action runs, as the toolbar's Mark done does (`runtime.setState`, "Marked … done · Undo").
- Actual:
  - The band shows "Mark done" and the row flies off and closes.
  - It then sticks at `data-phase="closing"` (height 0). No command is sent and the state is unchanged.
  - The row comes back on reload.
- Likely cause: `packages/ui/src/shell-list/Swipe.tsx:200-212` runs `onSwipe` only on a `height` transitionend, which never fires here.
- Shots: phone-midswipe.png, phone-afterswipe.png

### 9. [minor] The head's "Changes +n −n" adds up every turn's edits, not what's changed now
- Engine: Chrome and WebKit.
- Steps: a task whose first turn changed `math.js` (+1 −1) → two follow-ups → read the head chip.
- Expected: what the Changes pane and `git diff` show (1 file +1 −1).
- Actual:
  - "+3 −3" after two follow-ups; "+6 −6" after the goal's turns.
  - It still reads "+6 −6" after Revert file left nothing changed.
  - "Ship me" keeps "+1 −1" after its PR merged and its worktree was removed (Ship it… then says "Nothing to ship yet").
- Shots: chromium-changes-Addfix.png, chromium-addfix-waiting.png, chromium-after-merge.png

### 10. [minor] File links in answers open an empty Changes pane, not the file
- Engine: Chrome.
- Steps: the RICH answer → Files block's ↗ "Open in editor" on math.js; the code block's "math.js:1"; the tests block's "math.test.js:4".
- Expected: the file opens (file pane or editor), at the line.
- Actual: the Changes pane opens and says "No changes yet". `answer-ctx.ts:85` drops the path and line.
- Shots: chromium-Files_block_Open_in_editor.png

### 11. [minor] After ⌫ deletes the picked row, Enter opens the deleted task and toasts "no task <id>"
- Engine: Chrome and WebKit.
- Steps: Home → J to a row → ⌫ → Enter.
- Actual: it navigates to `/task/local:<deleted id>` with the raw error toast "no task 01a1f04…".

### 12. [minor] List keys L, M, ⇧R and V do nothing on the picked row
- Engine: Chrome and WebKit.
- Steps: Home → J to a row → L, M, ⇧R, V.
- Expected: as in v1 (`keys.js` acts on the focused row for l/m/t/R/F/s/v/p/d), and as the row menu's key hints suggest.
- Actual: no command, no dialog, no visible change. O, E, A and ⌫ do act on the row.

### 13. [minor] No Undo when you mark done with E, or archive from the task's ⋯
- Engine: Chrome; WebKit for E.
- Steps: (a) Home → J → E; (b) a task → ⋯ → Archive.
- Expected: "Marked done · Undo" / "Archived · Undo", as from the hover button and the row menu (and v1 `toggleDone`).
- Actual:
  - (a) the worktree dialog, and no toast.
  - (b) archived, then jumps to the next task, with no toast.

### 14. [minor] WebKit: opening a task page throws "ResizeObserver loop completed with undelivered notifications"
- Engine: WebKit 1280 only. 3 of 3 runs just opening a task page, plus after delete, restore and recipe. Chrome is clean.
- This misses the zero-console-warnings bar.

### 15. [minor] After "Drop goal", the goal's question stays and the task stays "Asking"
- Engine: Chrome.
- Steps: Set goal… (the fake can't meet it; three judged rounds run) → Task options → Drop goal.
- Actual:
  - "Needs you · The goal isn't met after 3 rounds… Keep going?" stays pinned above the reply box. The row says "Asking".
  - The file pane's Save is refused: "The agent is working on this task: wait for it, or stop it, then save." Nothing is running.
- Shots: chromium-addfix-waiting.png

### 16. [minor] The merge toast stops mid-sentence
- Engine: Chrome.
- Steps: PR strip → Merge → Merge PR, with local changes in the main checkout.
- Actual: "Merged PR #1. Local main was not updated: error: Your local changes to the following files would be overwritten by merge:" and the list of files is cut off.

### 17. [minor] Preview's Run does nothing when "Runs with" is empty
- Engine: Chrome and WebKit.
- Steps: side panel → Preview → Run, with the field empty (the placeholder reads "npm run dev").
- Actual: no command, no message, no DOM change for 4 s.
- Shots: webkit-preview-run.png

### 18. [minor] The task title says "Rename (M)" but clicking it does nothing, and it blocks window dragging
- Engine: Chrome and WebKit, desktop mode.
- Steps: click or double-click the title in the task head; then drag the head across the title.
- Actual:
  - No field and no dialog appear.
  - The title is `data-no-drag` and 444 px wide, so the middle of the head neither renames nor drags the window.
  - Only the head's top and bottom edges send `start_drag`.
- Shots: chromium-title-click.png

### 19. [minor] The side panel's place ⋯ offers only "Copy branch name"
- Engine: Chrome and WebKit.
- Steps: side panel → ⋯ ("Copy the path, open it elsewhere…").
- Expected: Copy path, Copy branch name, Open in editor, Open in terminal app. The row menu does open `vscode://file/<worktree>`.
- Actual: only "Copy branch name". `SidePanel.tsx:79` gets an empty `dir`.

## Notes (not counted)

- Delete from the row menu asks first and has no Undo: v1 parity.
- "Removed 1 folder · 16 KB" after Remove: v1 parity.
- E on a running task: "it is still working: stop it first". v1 said "Still working — stop it first or let it finish."
- Facts read "Claude Code · Default model" while the chip reads "Default" (fake harness, no model list).

## Cleanup

My scratch server (:4933, pid 46023) is stopped after this report. All data is under /tmp/qaC.
