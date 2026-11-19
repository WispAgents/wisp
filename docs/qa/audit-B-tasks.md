# Audit B: tasks on desktop, phone and Grid web (wisp v2 @ c0d655ad, 2026-11-19)

Findings only. No repo file was changed except this report.

**Setup.** This copies Audit A's setup (`docs/qa/audit-A-tasks.md`). Nothing real was touched.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-qaB3`. The worktree is at HEAD c0d655ad plus other sessions' uncommitted edits, and the build includes them. The UI is a `vite build` in `/tmp/qaB3/web`.
- **Server:** `wisp serve --port 4957` with `WISP_HOME=/tmp/qaB3/wisphome` and `HOME=/tmp/qaB3/home`. Scripts: `/tmp/qaB3/serve.sh` and `env.sh`.
- **Fakes:** every harness (`WISP_*_BIN`) points at the fake agent. The `claude` wrapper replays a rich answer for prompts containing RICH and fails for FAILME. The `codex` wrapper reads its version from a file, so the changed-harness watch can hold a turn. gh points at the fake gh.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`, and `WISP_GRID_URL` is the dev grid. A route check of Home, a task, My Stuff and Org (desktop and iOS) found **no request leaving localhost**. The Grid runs blocked every non-local request and none was attempted.
- **Grid:** `wisp-grid dev --port 4961 --data /tmp/qaB3/grid --wisp /tmp/qaB3/wisp`, signed in with dev handles `qa-b` and `qa-c`. Each got a cloud box that runs the same `wisp` binary and fakes.
- **Projects:** `app` (fake GitHub remote via `insteadOf`), `plain` (git, no remote), `notes` (no git).
- **Seeded tasks, every state:**
  - review, done, failed (FAILME), archived, pinned, for later, unread;
  - running (gated `run1`, with 1 queued follow-up);
  - waiting with a question (`runtime.ask`, options);
  - held (Codex binary changed while "Ask before running a changed harness" is on);
  - goal (`goal.set`);
  - compare (two runs in one `group`, as `wisp new --compare` makes them);
  - forked (`runtime.fork`);
  - crew (`crew.adopt` → qabot, `crew.create` → docbot);
  - scheduled (`later.add`);
  - rich blocks;
  - 14 fillers;
  - My Stuff: CSV, markdown, snippet and an app.
- **Caveat:** tasks made through `runtime.create` with a `cwd` run in the project's own checkout, with no worktree. That matches the CLI path (Audit A-setup F4). Tasks launched from Home, compare runs and forks got worktrees.
- **Browsers:** Playwright from `/tmp/pw` with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop mode is stubbed with `window.__TAURI_INTERNALS__`. Tauri calls are recorded (`start_drag`, `zoom_window`, `plugin:opener|open_url`).
  - Phones use an init script `window.Capacitor={isNativePlatform:()=>true,getPlatform:()=>'ios'|'android',Plugins:{}}`, at 390 and 360. Chrome uses CDP touch.
- **Feedback** is the ms from pointerdown, touchstart or keydown to the first DOM mutation. Logged steps: median 5 ms, p95 32 ms. Full-page Grid navigations can't be seen by a DOM observer, so they were timed to DOMContentLoaded instead.
- **Artifacts:**
  - Scripts: `/tmp/qa-keep/B/*.mjs`.
  - Raw step logs: `/tmp/qa-keep/B/results-*.jsonl` (303 logged steps). Many logged FAILs were my own selectors; each was re-checked by hand, and the tables below give the checked result.
  - Shots: `/tmp/qa-keep/shots-B/` (403 files).

## Counts

- **Action rows below: 83.** Many rows group several related actions, about 150 actions in all.
  - 70 pass.
  - 8 fail (a finding).
  - 5 not tested, each with why.
  - One more row is a note, not counted.
- **Findings: 10.** 0 critical, 3 major, 7 minor.
  - Two findings have no fail row of their own: #4 (console) and #7 (no Undo, inside a passing row).
- **Also re-checked from Audit A:**
  - Fixed: A #8 (phone swipes now run their action, with Undo) and A #18 (the task head's title now drags the window).
  - Still there: A #14 (WebKit `ResizeObserver loop` page error).
- Console in Chrome: clean on every surface.
- Untranslated keys: none found (scan of rendered text for `plugin.key.path` strings).

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows render, sections (Pinned, Needs you, Working, Scheduled, For later, Done, Archived) | pass | C W D 1280, 1440 L/D; no layout faults |
| Fold / unfold each section | pass | C W; 19→4 rows, fb 3 |
| Open row (click) | pass | C W; fb 16 |
| Hover Save for later + Undo | pass | C W; `forLater` set then cleared, fb 8 |
| Hover Mark done + Undo | pass | C W; done → review, fb 5 |
| Row ⋯ button opens the menu | pass | C W; fb 2 |
| Right-click menu (15 items) | pass | C W |
| Copy › flyout on hover (Copy task ID, Copy resume command) | pass | C W; clipboard and toast |
| Task options › flyout (Set goal, Side discussion, Add to crew, Save as recipe, Thread) | pass | C W; each opens its dialog or sheet, fb 1–4 |
| Pin / Unpin, Mark read / unread, Save for later + Undo | pass | C W; API state |
| Rename → Save | pass | C W |
| Archive + Undo | pass | C W |
| Delete → Cancel / → Delete | pass | C W; asks first; row and task gone |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C W; navigates, opens dialog, or `open_url vscode://file/…` |
| ⌘K → type → Enter | pass | C W; fb 6 |
| Project switcher (All ▾) → plain → ⌘0 | pass | C W |
| Hide / show the list ([) | pass | C W |
| Compare group → "vs Compare" | **fail** | C (#3) |
| Phone: rows, sections, crew stories | pass | C iOS/And 390, 360 L/D; W iOS 390 |
| Phone: search field → results | pass | C W iOS; fb 5–8 |
| Phone: project title ▾ → sheet → pick project | pass | C And 360 D; fb 5 |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C iOS 390; "Archived 2 tasks · Undo" restores both. W iOS: selection bar shows |
| Phone: long-press → menu / sheet | **fail** | C iOS 390/360, And 390/360 (#1); W iOS (right-click fallback) opens the menu |
| Phone: menu drill (Task options ›, and back) | pass | C iOS/And, W iOS; fb 2–3, "‹ Task options" back row |
| Phone: swipe right → Mark done + Undo | pass | C iOS 390; `runtime.setState`, "Marked done · Undo" (A #8 fixed) |
| Phone: swipe left → Save for later + Undo | pass | C iOS 390 |
| Phone: pull to refresh | pass | C iOS 390; indicator within 32 ms |
| Phone: swipes and pull in WebKit | not tested | Playwright has no touch injection in WebKit |
| Phone: iOS compact bar at 360 | **fail** | C iOS 360 D (#8) |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| Head: Changes chip → side Changes | pass | C W; fb 5; count matches `git status` |
| Side panel tabs (Changes, Shell; phone sheet: Changes, PR, Shell, Preview, Thread) | pass | C W D; C iOS/And |
| Shell: type `echo qa-$((40+2))` → `qa-42`; End shell | pass | C |
| Head Ship ▾ on a worktree task → "Ship it" dialog | pass | C |
| Head Ship on a task in the project's own checkout | **fail** | C (#2) |
| ⋯ Transcript (V) toggles the steps inline | pass | C W; fb 4 (Esc then goes Home, the app-wide "Esc back here") |
| ⋯ Open in › Open in Claude Code | pass | C; terminal side opens |
| ⋯ Rename, Pin / Unpin, Save for later | pass | C W |
| ⋯ Mark unread | **fail** | C W (#5) |
| ⋯ Fork…, Share…, Rerun… open | pass | C W |
| ⋯ Task options › Set goal, Add to crew, Save as recipe, Thread, Lean runs (radio, with its label) | pass | C W 1280 / C 1440 D |
| ⋯ Copy › Copy task ID / resume command | pass | C W |
| Reply: type → Enter → new turn | pass | C D; C iOS, And phone (`composer.send`), fb 2 |
| Reply box + menu (Attach files, Goal…, Delegate…, Start later) | pass | C W; fb 3 |
| Start later → In 10 min → "Starts at 01:47" chip | pass | C W; Send then schedules |
| Harness picker: Codex → "Follow up with Codex…"; kept after leaving and coming back | pass | C |
| Prompt bubble's harness mark after picking another harness | **fail** | C (#6) |
| Mode picker (Plan / Edits / Full auto) | pass | C W |
| Running task: live card, Stop, "1 follow-up queued" with Send now / Edit / Cancel | pass | C W D; C iOS/And |
| Waiting task: question card → option answers | pass | C D ("You answered: dayjs"); C And 390 D, fb 5 |
| Held task: "Codex changed since wisp last ran it · Skip · Trust and start" | pass | C W D; C iOS/And; buttons 40–44 px tall |
| Failed task: error and "Failed" pill | pass | C W |
| Rich answer (every block kind): layout | pass | C W 1280 |
| Phone: back button and system Back | pass | C W iOS; fb 5–9 |
| Phone: ⋯ sheet and its "This task ›" drill | pass | C iOS/And |
| Phone: live card header at 390 | **fail** | C And 390 D (#9) |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 (it would post to the wisp team) |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects (app, notes, plain, No folder, Other folder…) → pick | pass | C W; fb 7 |
| Place picker (This computer · Folder / Worktree / In place) | pass | C W; fb 3 |
| Mode, harness (two panes), + menu (Attach, Start from an issue, Recipes, Goal, Delegate, Compare with another agent, Start later) | pass | C W; fb 1–10 |
| Launch with Enter → task page at once, in its own worktree | pass | C W; fb 2; cwd `wisphome/worktrees/app-…` |
| Desktop drag strip: Home top → `start_drag`, double-click → `zoom_window` | pass | C W D |
| Desktop drag on the task head's middle (title) | pass | C W D (A #18 fixed) |
| Phone New task: full-width composer (374 of 390), input focused, Start → task | pass | C iOS/And 390/360, W iOS |
| Loading `/` restores the last view (Org) instead of Home | note | C D; Esc goes Home. Looks like intended session restore; not counted |

## My Stuff, crew, goals, later

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff: empty state; Add files (file chooser) | pass | C W |
| Open CSV viewer, markdown doc | pass | C W; fb 14–57 |
| Launch an app (iframe on its own port, runs) | pass | C W |
| Card menu (Open, Use in a new task, Copy, Download, Star, Edit details, Show in folder, Remove) | pass | C W |
| Remove + Undo | pass | C W; hidden at once, back after Undo |
| Phone Stuff tab | pass | C W iOS |
| Crew rail → member page; Org; Add to your crew | pass | C D; fb 6–9 |
| Phone Crew tab | pass | C W iOS |
| Goal task: goal band ("Done when") | pass | C D |
| Scheduled task row ("check CI again", time) | pass | C D (Scheduled section) |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp / a real harness |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Landing page, Dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: machines, box ⋯ (Copy address, Sleep now, Settings → Agents, API keys) | pass | C; fb 2 |
| Add ▾ (A computer of yours, Another cloud machine · Waitlist) | pass | C; fb 1 |
| Account page | pass | C; 154 ms to DOMContentLoaded (full page) |
| Getting started → Continue → the box's Settings › Agents | pass | C |
| Open the cloud box (asleep) → wisp UI | **fail** (feedback) | C (#10); box up in 1.8 s |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280; at 390 the box shows the phone list (FAB) |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost`; no hosts-file edits allowed |
| Shares from the Grid | not tested | no share on the dev grid |

## Findings

### 1. [major] Phone: a long-press opens the row menu under the finger, and lifting the finger runs the item there
- Engine: Chrome emulating the iOS and Android apps (Capacitor stub, CDP touch), at 390 and 360. 4 of 4 tries ran an action.
- Steps: Tasks → press and hold a row about 0.7 s → lift the finger without moving.
- Expected: the menu or sheet opens and stays. An item runs only on a new tap (iOS context menu, Material bottom sheet). v1 opened the sheet on long-press too.
- Actual: the item under the finger runs on release.

  | Platform, width | Item that ran | Effect |
  |---|---|---|
  | iOS 390 | Archive | "Filler 9" was archived ("Archived · Undo") |
  | iOS 360 | Mark read | `runtime.markRead` sent |
  | Android 390 | Pin | `runtime.pin`: "Filler 11" was pinned |
  | Android 360 | Rerun… | its dialog opened (`runtime.spec`) |

  Which item runs depends only on where the row was.
- Suspect: `packages/ui/src/shell-list/Swipe.tsx:44` dispatches the long-press as a `contextmenu` (so the menu opens while the finger is down). The ContextMenu / sheet around the list in `plugins/shell-list/ui/src/ui/TaskList.tsx` then takes that same touch's pointerup or click as a choice.
- Shots: `lp-android-holding.png` (the sheet under the finger), `phone-chromium-ios-390-light-longpress.png` (after: "Archived · Undo"), `lp-ios-holding.png`.

### 2. [major] Ship on a task that runs in the project's own checkout fails with a raw "no such session" toast
- Engine: Chrome and WebKit, desktop 1280.
- Steps: a task launched with a `cwd` and no worktree (CLI, MCP or `runtime.create`, e.g. "Done one" in `/tmp/qaB3/repos/app`) → head → Ship.
- Expected: the Ship it dialog (commit, branch, PR). Or, if shipping needs a worktree, a control that says why. A worktree task opens "Ship it · wisp/task-… · origin/main…" fine.
- Actual:
  - `ship.view {session}` fails.
  - The toast reads `no such session: 01a1f12e3c4f706abca580fbc26c1707`.
  - Nothing opens.
  - The Ship button is offered on every such task.
- Suspect: `plugins/ship/host/src/ship.rs:115` (`Tasks` only tracks worktree tasks) and `plugins/ship/ui/src/ShipButton.tsx:28`, which shows the button anyway.
- Shots: `task-chromium-1280-light-head_Ship_.png`, `ship-done-chromium.png`.

### 3. [major] Compare says "These runs are gone." for a compare group whose runs are all there
- Engine: Chrome, desktop 1280.
- Steps: two runs of one ask in one group (as `wisp new --compare claude,codex` makes them: `ext.wisp.runtime.group`), both finished. The list shows them as one Compare card → press "vs Compare".
- Expected: the side-by-side table (time, cost, changes, tests) with Keep this.
- Actual: the Compare modal is empty except "These runs are gone."
- Suspect: `packages/shell/src/frame/Compare.tsx:66`. It finds the run by `props.params.id`, which arrives as `local:01a1f12e44fc…` (the overlay URL is `o=dialog:shell.compare:{"id":"local:…"}`). The sessions in the store are matched by id, so the lookup likely misses the peer prefix. The list's own grouping finds the group, so the data is there.
- Shot: `home-chromium-1280-light-compare.png`.

### 4. [minor] WebKit still throws "ResizeObserver loop completed with undelivered notifications" (A #14)
- Engine: WebKit only, desktop 1280/1440 and iOS phone 390. Seen on every page load (Home, task, My Stuff). Chrome is clean.
- WebKit also warns that `TaskSide-*.js`, `Tabs-*.js`, `StuffPage-*.js`, `items-*.js` and `AppFrame-*.js` were "preloaded using link preload but not used within a few seconds".
- Expected: zero console warnings.

### 5. [minor] "Mark unread" in an open task's ⋯ jumps to another task with no message
- Engine: Chrome and WebKit, desktop 1280.
- Steps: open "Pinned one" → ⋯ → Mark unread.
- Expected: the task is marked unread. If wisp must leave it (opening marks it read), it says so.
- Actual:
  - The page switches to a different task at once, with no toast.
  - In a run that went on with ⋯ → Save for later, the action landed on that other task ("Filler 13" was saved for later instead of "Done one").
- Suspect: the head menu's Mark unread path in `packages/shell/src/task-menu.ts` / `packages/shell/src/builtins.tsx:168` (open marks read). v1's behaviour here not confirmed.
- Shot: `unread-chromium.png`.

### 6. [minor] Picking another harness in the reply box changes the harness mark on earlier prompts
- Engine: Chrome, desktop 1280.
- Steps: a task whose turn 1 ran on Claude Code → reply box harness chip → Codex → Default (Codex's).
- Expected: earlier prompt bubbles keep Claude Code's mark. The turn ran there: `runtime.turns` says `agent: claude`, and the facts read "Claude Code · Default model".
- Actual: turn 1's bubble shows the Codex mark as soon as Codex is picked, before anything is sent.
- Suspect: the user prompt bubble's logo in the task view (`packages/ui/src/app/Task.tsx` / `plugins/shell-task/ui/src`) reads the composer's runner pick, not `turn.agent`.
- Shot: `harness2-after-model.png`.

### 7. [minor] Head ⋯ "Save for later" gives no Undo, while the row menu and hover do
- Engine: Chrome and WebKit.
- Steps: open a task → ⋯ → Save for later.
- Expected: "Saved for later · Undo", as from the hover button and the row menu, and as A #13 asks for Archive.
- Actual: the task is saved for later (`forLater:{at,by}`) with no toast and no Undo. The item then reads "Unsave".
- Shot: `probe3-saveforlater.png`.

### 8. [minor] iOS phone at 360: the collapsed task-list bar runs the title into the search button
- Engine: Chrome emulating the iOS app, 360 × 844, dark (also visible in light).
- Steps: Tasks → scroll the list so the large title collapses into the bar.
- Expected: "All projects ▾" and the search icon side by side, with room between them (iOS nav bar).
- Actual: "All projects" ends against the magnifier with no gap, and the ▾ is gone. At 390 they nearly touch.
- Suspect: `packages/ui/src/css/platform/ios.css` (phone tasks bar, compact title).
- Shot: `phone-chromium-ios-360-dark-longpress.png` (top bar).

### 9. [minor] Phone: the live card's status is cut to "Claude Code is waiti…"
- Engine: Chrome emulating Android, 390 dark.
- Steps: a running task asking a question → the task page.
- Actual: the card header reads "Claude Code is waiti… 35m 25s 3 actions". The status word, the one thing it says, is truncated, while the timer and the action count keep their space.
- Expected: the status kept whole (the timer or the count drops first), per design/layout.md's priorities.
- Shot: `ask-chromium-android-dark.png`.

### 10. [minor] Grid: Open on a sleeping box shows nothing for about 1.3 s
- Engine: Chrome, Grid at 1280.
- Steps: Grid Home → Cloud wisp "Asleep · starts the first time you open it" → Open.
- Expected: a visible change within 100 ms: Open's pending state, or a "Waking your wisp…" page.
- Actual: the Grid page stays as it was, with no pressed or pending state, until the box answers about 1.3 s later. The app is up at 1.8 s.
- Grid page links (Account) take 80–154 ms to DOMContentLoaded, also with no pending state.
- Suspect: the Open link on the Grid home (`apps/grid/src/edge_pages.rs` / `assets`) is a plain navigation.

## Notes (not counted)

- Loading `/` restores the last main view (Org) rather than Home; Esc goes Home. Looks like intended session restore.
- Desktop Esc from an inline Transcript leaves the task for Home (the app-wide "Esc back here"). It doesn't close the transcript first.
- iOS long-press opens a popover menu with no "Select" row; Android's sheet has "Select" first. Select is still in the bar ⋯ on both.
- Tasks started through `runtime.create` / the CLI with a `cwd` run in the project's own checkout with no worktree (Audit A-setup F4). That is what makes #2 reachable.
- The Grid has a few icon buttons with no accessible name (seen in the DOM dump of Grid Home). Not checked further.

## Cleanup

My two servers, both started by me, are stopped after this report:
- `wisp serve` :4957 (pid 20229);
- `wisp-grid dev` :4961 (pid 22150), which stops its boxes on exit.

No other process was touched. Data stays under `/tmp/qaB3` and `/tmp/qa-keep`. `/tmp/wisp-target-qaB3` is kept for a re-run, since rebuilding took 35 minutes; delete it when done.
