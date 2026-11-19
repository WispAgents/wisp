# Audit E: tasks on desktop, phone and Grid web, with the task side panel (wisp v2 @ e0cb033e, 2026-11-19)

Findings only. No repo file was changed except this report.

**Setup.** This copies Audit D's setup (`docs/qa/audit-D-tasks.md`) and the side panel audit's states
(`docs/qa/audit-side-panel.md`). Nothing real was touched: no real harness, no service, and never ports 4477, 4800 or
4791 or `~/.wisp*`.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`
  (incremental, 47 s). HEAD e0cb033e, plus other sessions' uncommitted edits under `design/v1-reference/`. The UI is a
  `vite build` in `/tmp/qaE3/web`.
- **Server:** `wisp serve --port 4943` with `WISP_HOME=/tmp/qaE3/wisphome` and `HOME=/tmp/qaE3/home`. Scripts:
  `/tmp/qaE3/serve.sh` and `env.sh`. Of 4900–4999, only 4962 was busy (another auditor's); I used 4943 and 4947.
- **Fakes:** as in D. Every harness points at the fake agent. The `claude` wrapper replays a rich answer for RICH and fails
  for FAILME. Appending a line to the `codex` wrapper changed its fingerprint, which made the changed-harness watch hold
  turns. gh points at the fake gh.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`, and `WISP_GRID_URL` is the dev grid.
- **Grid:** `wisp-grid dev --port 4947 --data /tmp/qaE3/grid --wisp /tmp/qaE3/wisp`, dev handles `qe-one`, `qe-two` and
  `qe-three`.
- **Projects:** `app` (fake GitHub remote via `insteadOf`), `plain` (git, no remote), `notes` (no git), made fresh from D's
  files.
- **Seeded tasks:** D's seed scripts (`/tmp/qa-keep/E/seed1-3.mjs`, `seedstuff.mjs`, `seedE.mjs`), every state:
  - review, done, failed (FAILME), archived, pinned, for later, unread;
  - running (gated `run1`, `run2`, `run3`; `run1` and `run2` with a queued follow-up);
  - two asking questions (`runtime.ask` on gated `ask1`, `ask2`);
  - held: two Codex turns ("Codex task", Compare B) held by one wrapper change;
  - goal, forked, crew (qabot adopted, docbot created), scheduled (`later.add`), rich blocks, 14 fillers;
  - My Stuff: CSV, markdown, snippet and an app;
  - **compare group** `g-qa1` (Compare A on Claude Code, Compare B on Codex), plus the CLI compare
    `wisp "TOPIC[CLI compare] Make add faster" --agent claude --compare codex`. Every compare run got its own worktree.
- **Side panel states** (as in the side panel audit):
  - a worktree with real edits (Compare A: `math.js` changed, `extra.txt` added);
  - a task in place while the project checkout is dirty (Review me; `README.md` edited by "the user");
  - a task whose worktree was removed (Forked review; `git worktree remove`);
  - a folder without git (Notes one);
  - an open PR: Ship → Commit & open PR on the CLI compare's Claude run gave PR #1 on the fake gh;
  - a My Stuff app pinned beside tasks (Counter app, Pin beside tasks);
  - the Thread tab.
- **Browsers:** Playwright from `/tmp/pw`, with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop is stubbed with `__TAURI_INTERNALS__`. `window.open` is recorded, since links out now open in the click.
  - Phones use the Capacitor stub (`ios` / `android`) at 390 and 360. Chrome uses CDP touch.
  - Light and dark.
  - Every browser is closed in a `finally`; D's scripts close theirs through an exit handler added to `lib.mjs`.
- **Feedback** is the ms from pointerdown, touchstart or keydown to the first DOM mutation. Over about 240 logged steps:
  median 6 ms, p95 33 ms. Over 100 ms:
  - 409 and 414 ms: my script clicking the launcher's "in" label (as in C and D);
  - 504–507 ms: long-press on iOS and Android (see Notes).
- **Artifacts:**
  - Scripts: `/tmp/qa-keep/E/*.mjs`. These are D's scripts with paths and ports swapped, plus hand probes `e1.mjs`–`e34.mjs`.
  - Raw step outputs: `/tmp/qa-keep/E/*.out`.
  - As in C and D, many scripted FAILs were my own selectors: the list is virtualized, rows near the bottom sit under the
    phone FAB, and the iOS script tapped the collapsed title. Every FAIL was re-checked by hand, and the tables give the
    checked result.
  - Two stray follow-ups ("echo chromium-qa…", "echo reattached…") went to Compare A when my script typed into the reply
    box instead of the terminal. They are seed noise.
- **Shots:** `/tmp/qa-keep/shots-E/` (557 files).

## Counts

- **Action rows below: 105.** Many rows group several related actions, about 200 actions in all.
  - 88 pass.
  - 12 fail: one for each NEW finding (#1–#11), plus Audit D #1, which still fails.
  - 5 not tested, each with why.
- The re-check tables (Audit D's 5, the side panel audit's 12) come on top of these rows.
- **Audit D's 5 findings, re-checked:** 4 fixed (#2, #3, #4, #5); 1 still fails (#1).
- **Side panel audit's 12 findings, re-checked:** all 12 hold (pass). Two new side panel findings sit next to #2 and #4
  (#5 and #6 below).
- **The strip at 1280 and 1440 (the brief's check): pass.**
  - Without a pinned tab, Chrome and WebKit, light and dark: 1280 shows Changes, PR, Shell and Preview labelled, with
    Thread as its icon (290 of 297 px). 1440 labels all five (380 of 389 px).
  - With a PR, Changes and PR show their counts ("Changes 1", "#1"), and Thread gives way first.
  - With a pinned app (closed) at 1440 all four stay labelled. At 1280 Preview turns to its icon after the pinned tabs and
    Thread, as designed.
  - The case that fails is a pinned tab opened at 1280 (#6).
- **NEW findings: 11.** 0 critical, 3 major, 8 minor (#1–#11 below).
- Console: clean in Chrome and WebKit on every surface. Untranslated keys: none.

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## Audit D's findings, re-checked

| D # | Finding | Now | How checked |
|---|---|---|---|
| 1 | Home's Needs you leaves out the task asking you a question | **still fails** | C D 1440 L, 1280 L; W D 1440 D. Two loads, before and after ask1 was answered:<br>• With ask1 and ask2 asking, the sidebar lists both first. Home's "5 of 29" shows ask2 second, as "Waiting", and leaves out ask1.<br>• With only ask2 asking, the sidebar puts it first. Home's five leave it out (a stopped run and four finished ones).<br>• The fix in `plugins/shell-home/ui/src/needs.ts` sorts by `question`/`approval`. A `runtime.ask` row arrives with `state: "waiting"`, `question: ""`, `approval: false` and `asks: false` (from `runtime.list`), so it never counts as asking. v1's key was the `waiting` state.<br>Shot `e1-home-chromium-1440-light.png` |
| 2 | A held row says "Waiting for codex" | **fixed** | C W D 1280/1440, phones: "Waiting for Codex" on "Codex task" and Compare B. Shot `e2-side-chromium-1280-light.png` |
| 3 | With the side panel open, the head details strip cuts the harness name | **fixed** | C W D 1280 and 1440: "Claude Code · Default model · app · wisp/task-9d8785", none cut (260/260 and 160/160 px at 1280). Shot `e3-side-chromium-1280-light.png` |
| 4 | Phone: a scheduled new task's title is out of line | **fixed** | C iOS 390 L, And 360 D: planned rows' titles start where task titles start. A different misalignment in the same section is #11. Shot `d20-ios-390-light-bottom.png` |
| 5 | Android: My Stuff's reader is a tinted card with an inset header | **fixed** | C And 390 L: one full-screen page with its own header, as on iOS. Shot `pstuff2b-android-QAnotes-touch.png` |

## Side panel audit's findings, re-checked

| SP # | Finding | Now | How checked |
|---|---|---|---|
| 1 | The strip hid v1's tabs behind `+` | **holds** | C W D 1280/1440: Changes, PR, Shell, Preview, then pinned and Thread; no `+` |
| 2 | Labels hidden under 420 px | **holds** | See Counts: four labelled at 1280, all at 1440 |
| 3 | A pinned tab's label squeezed the panel's own | **holds** while the pinned tab is closed (it goes to its icon first). When it is open: #6 |
| 4 | `→` moved one tab, then focus fell to the page | **holds** for Changes → PR → Shell → Preview (C W). Moving on to Thread or a pinned tab still drops focus: #5 |
| 5 | Background reads toasted raw errors | **holds** | C W D: no toast on the worktree-gone, in-place and no-git tasks |
| 6 | Changes on a task in place diffed the whole checkout | **holds** | C W D: Review me lists only math.js (edits wisp saw), not the user's `README.md` edit in the checkout |
| 7 | Changes on a task whose worktree is gone said "No changes yet" | **holds** | C W D: Forked review shows the edits wisp saw, with no toast |
| 8 | Open file… on a folder git can't list | **holds** | C W D: not offered on the in-place, gone and no-git tasks. On git's diff it opens, but see #1 |
| 9 | End shell ended at once | **holds** | C W D: "End the shell? Whatever runs in it stops with it. · Cancel · End shell", fb 2–7. Cancel keeps it; End → "Start again" |
| 10 | A reattached shell kept its old size | **holds** | C W D: a shell started at 1440 rewraps to the 395 px pane at 1280. No element past the edge. Shot `e12-shell-1280-reattach-chromium.png` |
| 11 | The Shell in a dark box inside a white margin | **holds** | C D: terminal edge to edge, `zsh · End shell` foot. Shot `e4-chromium-1440-light-tab_Shell.png` |
| 12 | Head Changes chip on a task in place showed the checkout's diff | **holds** | C W D: no chip on in-place, gone and no-git tasks. On the worktree task "+3 −1" matches the panel |

## Side panel

| Action | Result | Where / how checked |
|---|---|---|
| `]` opens, `]` and × close | pass | C W D; fb 5–12. Focus must be outside the reply box, which is focused on load. With focus in the box, `]` types a bracket (expected) |
| Head button toggles the panel | pass | C W D; fb 5–12. Its label stays "Show side panel (])" while open (aria-pressed true); see Notes |
| Strip at 1280 / 1440, no pins | pass | C W D L/D (see Counts) |
| Strip with a PR and a count | pass | C W D 1280/1440: "Changes 1 · #1 · Shell · Preview · [Thread]" |
| Strip with a pinned app, closed | pass | C W D 1440 all labelled; 1280 Preview to its icon after pinned and Thread |
| Strip with the pinned app open, 1280 | **fail** | C W D (#6) |
| Each tab opens (Changes, PR, Shell, Preview, Counter app, Thread) | pass | C W D; fb 0–1 |
| `←` / `→` walk the tabs | **fail** | C W D (#5) |
| Changes on a worktree: list, fold a diff, Collapse all | pass | C D; fold fb 1 |
| Changes header words on a git diff | **fail** | C W D, phones (#4) |
| Changes on a task in place / worktree gone / no git | pass | SP #6, #7 |
| Changes that wisp saw, two hunks in one file | **fail** | C W D (#10) |
| Open file… → pick a file | **fail** | C W D (#1) |
| `⋯` lists Copy path, Copy branch name, Open in editor, Open in terminal app | pass | C W D |
| Copy path, Copy branch name | **fail** | C W D: the clipboard gets the worktree path and `wisp/task-9d8785`, with no word on screen (#7) |
| Open in editor | pass | C D: opens in the click (`window.open`, see 04cd03b7); the Tauri stub records nothing |
| Open in terminal app | not tested | Refused with "No terminal app on this computer…". The server counts itself headless because my env sets `WISP_GRID_URL` (`apps::in_wisp`), so this is my setup (see Notes) |
| Resize by the grip (1440) | pass | C D: 480 → 556 px on a 120 px drag (the task column keeps its minimum) |
| PR: card, checks, Merge asks first (cancelled) | pass | C W D 1280/1440: "feat: fake claude finished · #1 · Open · Ready to merge · Merge · Checks · Open on GitHub". Merge asks "Merge this pull request? It squash-merges…", fb 2; Esc cancels |
| PR tab label | **fail** | C W D (#8) |
| Ship → Commit & open PR | pass | C D: fb 12, "Opened PR #1", `ship.ship` |
| Shell: start, type, End shell asks, Start again | pass | SP #9 |
| Shell reattached at another width | pass | SP #10 |
| Preview: says how to run; Run offered | pass | C W D: "Run the app from this task's folder and see it here · Runs with · Run" |
| Pinned My Stuff tab opens its app | pass | C W D: "Counter app" with its +1 button, fb 1 |
| Thread tab | pass | C W D: Notes headings (Goal, Constraints, Decisions…) |
| Esc with the panel open | pass | C D: Esc closes the panel, a second Esc leaves the task (as before) |
| Phone sheet: tabs, Changes | pass | C iOS 390 L (Changes, PR, Shell, Preview, Counter app, Thread); fb 7 |
| Phone sheet strip at 360 with a pinned tab | **fail** | C iOS 360 D, And 360 D (#2) |
| Phone Changes file rows on a worktree | **fail** | C iOS/And 360 and 390 (#3) |

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows render; sections Pinned, Needs you, Working, Scheduled, For later, Archived | pass | C D 1280 L, W D 1440 D; scrolled to the end: Working 3, Scheduled 4, For later 1, Archived 1 |
| Fold / unfold a section | pass | C W; Needs you 26: 13 → 6 rows, fb 4 |
| Open a row (click) | pass | C W, by hand; fb 14–21 |
| Hover Save for later + Undo | pass | C W, by hand; fb 7–11 |
| Hover Mark done + Undo | pass | C W, by hand; fb 4–7. The worktree dialog that follows is #9 |
| Mark done on a worktree task: "Remove its worktree?", then Undo | **fail** | C W D 1280 (#9) |
| Row ⋯ menu | pass | C W; fb 2 |
| Copy › (task ID, resume command) | pass | C W; toast, clipboard |
| Task options › Set goal, Side discussion, Add to crew, Save as recipe, Thread | pass | C W; fb 1–2 |
| Pin / Unpin | pass | C W, by hand: Pinned 1 → 2, the row moves to the top |
| Mark read / unread, Save for later + Undo, Archive + Undo | pass | C W; fb 4–10 |
| Rename → Save | pass | C ("renamed"). W's step lost its row to the virtualized list |
| Delete → Cancel / → Delete | pass | C (W's Delete hit a row below the fold, as in D) |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C W; navigates or opens a dialog |
| ⌘K → type → Enter | pass | C W; fb 6 |
| Project switcher (All ▾) → plain → ⌘0 | pass | C W; fb 4–10 |
| Hide / show the list ([) | pass | C W; fb 0 |
| Compare group → vs Compare → table → Keep this → confirm | pass | C D 1280: table fb 6, ask fb 2, confirm fb 12, "Kept Claude Code's run. Ship it when you're ready.", Compare B archived |
| Held rows | pass | D #2 fixed |
| Phone: rows, sections, crew story, bottom tabs | pass | C iOS 390 L, And 360 D, W iOS 390 D; tabs fb 3–6 |
| Phone: search → results | pass | C W iOS; fb 6–7 |
| Phone: project title ▾ (large title, and the small one after scrolling) → Projects sheet | pass | C W iOS 390/360 (fb 6–10), C And 360/390 (fb 6). The scripted FAIL was my tap on the hidden small title |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C And 360 ("Archived 2 tasks · Undo", both back), W iOS 390 (bottom toolbar) |
| Phone: long-press → sheet / menu, drill into Task options | pass | C iOS 390, And 360; menu at 504–507 ms, drill fb 2 |
| Phone: swipe right → Mark done + Undo | pass | C iOS 390 fb 33, And 360 fb 32 |
| Phone: swipe left → Save for later + Undo | pass | C iOS 390 fb 31–32, And 360 fb 32 (the scripted FAIL started the swipe on the FAB) |
| Phone: pull to refresh | pass | C iOS fb 34, And fb 33 |
| Phone: Scheduled section times | **fail** | C iOS 390, And 360 (#11) |
| Phone: swipes and pull in WebKit | not tested | no touch injection in WebKit |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| Head: Ship on a worktree task → "Ship it" dialog | pass | C; fb 7 |
| No Ship on a project-checkout task | pass | C W (deliberate) |
| ⋯ menu (Transcript, Shell, Rerun, Fork, Open in, Rename, Pin, Mark unread, Save for later, Task options, Copy, Share, Archive, Delete) | pass | C W; fb 1–4 |
| ⋯ Save for later + Undo | pass | C W, by hand: "Saved for later · Undo", fb 10–21, the page stays |
| ⋯ Mark unread + Undo, Pin / Unpin | pass | C W |
| ⋯ Transcript (V) | pass | C W; fb 4–6. Esc then goes Home (note, as in B–D) |
| Reply: type → Enter → new turn | pass | C D; C iOS/And fb 2 |
| Reply + menu, mode picker, harness picker | pass | C W; fb 1–9 |
| Start later → time → Enter (task page and Home) | pass | C W: focus returns to the box; Enter sends `later.add` (task) or `later.plan` (Home) |
| Queued follow-up: hover shows Send now / Edit / Cancel; Edit → Enter saves | pass | C D: fb 9, `runtime.queue.text`, "(edited)" saved |
| Stop a running turn | pass | C D: now asks "Stop Claude Code? The run ends where it is." Stop → review, "You stopped this run. Follow up to pick it back up." · Partial, fb 3 |
| Waiting task: question card → answer | pass | C D (the sweep answered ask1 with "dayjs"; the card shows "You answered: dayjs") |
| Held task: "Codex changed since wisp last ran it · Skip this run · Trust and start" | pass | C D: fb 8, one `runtime.release` started both held turns (Codex task and Compare B). Phone buttons 44 px (iOS) / 40 px (Android) |
| Failed task, rich blocks, goal band | pass | C W D, phones; goal band "Done when … · Working · Round 0" |
| Head details strip with the side panel open | pass | D #3 fixed |
| Phone: back button and system Back | pass | C W iOS, C And; fb 5–8 |
| Phone: ⋯ sheet and its drills | pass | C iOS/And; fb 1–4 |
| Phone: reply box full width + send | pass | C iOS (24 to 366), And (24 to 336) |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects → pick | pass | C, by hand; fb 5–7 (the scripted 409/414 ms was my click on "in") |
| Place, mode, harness, + menu | pass | C W; fb 1–9 |
| Launch with Enter → task page at once, in its own worktree | pass | C W; fb 2 |
| Needs you on Home | **fail** | D #1 |
| Top bar switcher → Manage projects… | pass | C W: fb 7–12. Shows "This computer · Machine · Browse… · Add", three projects with task counts and delete. Shot `e28-manage-chromium.png` |
| Desktop drag strip: Home top, task head middle | pass | C W D: `start_drag`, double-click `zoom_window` |
| Phone New task → Start → task page | pass | C iOS 390, And 360; fb 4–5 |

## My Stuff, crew, goals, later

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff layout; Add files | pass | C W D |
| Open CSV and markdown | pass | C W D; fb 16–61 |
| Launch an app | pass | C W D; fb 3 |
| Card menu (Launch, Pin beside tasks, Pin on home, Use in a new task, Star, Edit details…, Show in folder, Remove) | pass | C W D; fb 3 |
| Pin beside tasks | pass | C D: "Pinned Counter app beside tasks · Undo", fb 5. It shows in the side panel strip |
| Remove + Undo | pass | C W D |
| Search filters | pass | C W D; "Counter" matches the app and its copies (as in C, D) |
| Phone Stuff: tabs, open a doc | pass | C iOS/And: the reader opens in the app, fb 66–69 |
| Crew rail → member page; Org; Add to your crew | pass | C W D; fb 6–9 |
| Goal task: goal band | pass | C W D, phones |
| Scheduled section (desktop): rows open their task | pass | C D: Scheduled 4, fb 19 |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp or a real harness (as in C, D) |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Landing and dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: box ⋯ (Copy address, Settings → Agents, API keys; Sleep now once up) | pass | C 1280, 390; fb 2 |
| Add ▾ (A computer of yours, Another cloud machine · Waitlist) | pass | C 1280, 390; fb 1 |
| Account page | pass | C 1280, 390 (a full-page load; the script's fb is null, as in D) |
| Getting started → Continue | pass | C 1280, 390; opens the box (full-page load) |
| Open the cloud box (asleep) → wisp UI | pass | C 1280, fresh `qe-three`: Open carries `data-wake="Waking up · a few seconds…"`, and the box UI was up within 90 ms of the click |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280. At 390 the box shows the phone list (FAB), as in D |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost` (as in C, D) |

## NEW findings

### 1. [major] After Open file… picks a file, the focus is left on the page: typing "e" marks the task done
- Engine: Chrome and WebKit, desktop 1440. 2 of 2.
- Steps: open a worktree task's side panel (Changes) → Open file… → type `math` → Enter. The file opens in the panel ("math.js · Saved"). Then type `e`.
- Expected: the focus goes to the opened file, or back where it was. Keys typed after the pick never run the list's shortcuts.
  This is the case the side panel audit's #8 fixed for a refused list ("typing 'peers' marked the task done with e").
- Actual:
  - After the pick, `document.activeElement` is `BODY`.
  - `e` sends `runtime.setState`: toast "Marked done · Undo", and the task's state is `done`.
  - Any other single-key shortcut fires the same way.
- Shots: `e5-filepane-chromium.png` (the file open, focus on the page), `e7-after-pick-e-chromium.png`, `e7-after-pick-e-webkit.png`.

### 2. [major] Phone side sheet: with a pinned tab, the strip runs past the screen at 360 and Thread is cut off
- Engine: Chrome iOS 360 D, Android 360 D (also Android 390 by 2 px). Counter app pinned beside tasks.
- Steps: a phone at 360 → a task → its files → the sheet.
- Expected: every tab reachable. The strip fits, scrolls, or turns tabs into icons as on desktop (design/layout.md: nothing
  clipped).
- Actual:
  - The tab list keeps its 361 px (iOS, x 16–377) or 386 px (Android, x 6–392) on a 360 px screen. It doesn't scroll (`scrollWidth == clientWidth`).
  - iOS shows "Th" at the edge. On Android, Thread is outside the screen: only a sliver shows.
- Shots: `e29-phone-sheet-ios-360-dark.png`, `e29-phone-sheet-android-360-dark.png`, `e29-phone-sheet-ios-390-light.png` (390 just fits).

### 3. [major] Phone Changes on a worktree task: file names cut to two letters, with the count drawn over them
- Engine: Chrome iOS and Android, 360 and 390. Desktop is fine (1280: "extra.txt" 48 px whole).
- Steps: a worktree task with changed files (Compare A) on a phone → its files → Changes.
- Expected: the file name is the row's main text, readable, with the counts and the Stage / Viewed boxes after it.
- Actual:
  - The name gets a 24 px box: "extra.txt" and "math.js" read "ex." and "m." (`scrollWidth` 48 > `clientWidth` 24).
  - At 360 the "+1" count box (x 63–98) is drawn over the name (x 70–94).
  - Meanwhile Stage and Viewed take the middle of the row.
  - The diff lines also show the blue "+" comment button over the first character ("+new file").
- Shots: `e29-phone-sheet-android-360-dark.png`, `e29-phone-sheet-ios-360-dark.png`.

### 4. [minor] Changes on a worktree says "edits wisp saw / Not a git diff" over git's own diff
- Engine: Chrome and WebKit desktop 1440; phones. Read after 6 s, so it isn't the moment before git answers.
- Steps: a worktree task with changes (Compare A) → side panel → Changes.
- Expected: "vs origin/main" (`shell-task.changes.vs`). The list is git's: it shows `extra.txt`, which no turn reported,
  with Stage, Commit and Open file…, which are offered only on git's diff.
- Actual:
  - The header reads "2 files +3 −1 edits wisp saw", with the title "Not a git diff: the edits wisp saw the agent make".
  - The words say the opposite of what the list is.
- Suspect: `plugins/shell-task/ui/src/side/ChangesPane.tsx:385` chooses the label with `git()`, while the list (`files`, line 159) shows git's rows. The `labels` object may be read once before the resource resolves.
- Shot: `e4-chromium-1440-light-changes.png`.

### 5. [minor] `→` on the strip drops focus to the page when it reaches Thread or a pinned tab
- Engine: Chrome and WebKit desktop 1440.
- Steps: focus the Changes tab, press `→` four times.
- Expected: Changes → PR → Shell → Preview → Thread, with focus on each tab (side panel audit #4).
- Actual:
  - PR, Shell and Preview take focus.
  - The next `→` opens Thread (the URL becomes `?side=view:shell-task.tab`), and focus falls to `BODY`.
  - The next `→` does nothing.
  - The same happens when moving onto a pinned tab (also a `view:` tab).
- Shot: `e5-arrows-chromium.png`.

### 6. [minor] At 1280, opening a pinned tab turns Changes, PR, Shell and Preview into bare icons while there is room
- Engine: Chrome and WebKit desktop 1280 (panel 396 px, 297 px for the strip).
- Steps: pin a My Stuff app beside tasks → open a task's panel → click the app's tab.
- Expected: the open tab keeps its label, and the panel's own tabs keep theirs as far as room allows. Pinned tabs and Thread
  give way first, which is the owner's 2026-11-19 rule and the brief's.
- Actual:
  - The strip becomes "[Changes] [PR] [Shell] [Preview] Counter app [Thread]", using 252 of 297 px.
  - Changes also loses its count.
  - About 45 px stay empty: enough for Shell's label (+28 px), so the plan doesn't use the room.
  - With the pinned tab closed, the same strip keeps Changes, PR and Shell labelled.
- Shots: `e16-pinned-open-1280.png`, `e34-webkit-pinned-open-1280.png`.

### 7. [minor] Copy path and Copy branch name in the panel's ⋯ say nothing
- Engine: Chrome and WebKit desktop.
- Steps: side panel ⋯ → Copy path (or Copy branch name).
- Expected: a toast, as v1 (`web/js/features/panel.js:804-805`: "Path copied", "Branch name copied") and as the list's own Copy items ("Copied the ID of …").
- Actual:
  - The menu closes and the clipboard has it, but nothing confirms it. No toast at 250 ms or 1.25 s.
  - `SidePanel.tsx:88-89` calls `shell.platform.copy` without one.
- Shot: `e4-chromium-1440-light-copy_path.png`.

### 8. [minor] The PR tab reads "#1" without "PR"
- Engine: Chrome and WebKit desktop 1280/1440; phone sheet.
- Steps: a task with an open PR → side panel.
- Expected: v1's tab name "PR #1" (`panel.js:231`: `` `PR #${number}` ``).
- Actual:
  - The tab is the merge icon and "#1". The word PR only shows in the tooltip (`SidePanel.tsx:162` replaces the label).
  - At 1440 there is room for it: 346 of 389 px used.
- Shot: `e15-pr-chromium-1440-dark.png`.

### 9. [minor] Undo after Mark done leaves "Remove its worktree?" open, offering to remove the folder of a task that's no longer done
- Engine: Chrome and WebKit desktop 1280. 2 of 2.
- Steps: hover a worktree task's row → Mark done. The toast "Marked done · Undo" and the dialog "Remove its worktree? · Keep ·
  Remove" open together. Press Undo in the toast; it can be reached.
- Expected: Undo puts the task back and takes the question with it.
- Actual:
  - The task goes back to review, but the dialog stays.
  - Its Remove would remove the reopened task's worktree. The worktree was clean here, and its branch stays.
- Shots: `e27-markdone-ask-chromium.png`, `e27-after-undo-dialog-chromium.png`.

### 10. [minor] Changes that wisp saw show "⋯ 0 unchanged lines" between two hunks
- Engine: Chrome and WebKit desktop 1440.
- Steps: a task in place with two turns that each edited `math.js` (Review me) → side panel → Changes.
- Expected: no fold row for zero lines; adjacent hunks run on, or the rows join.
- Actual: a "⋯ 0 unchanged lines" row between the two hunks. It folds nothing.
- The fake's two edits are the same edit, so the content is seed. The zero-line fold row is not.
- Shot: `e8-chromium-in_place_Review_me.png`.

### 11. [minor] Phone: a task row's clock time in Scheduled sits about 25 px left of every other time
- Engine: Chrome iOS 390 L, Android 360 D. Desktop is aligned (all end at x 372).
- Steps: the Tasks list on a phone → scroll to Scheduled (tasks with a follow-up scheduled, and planned new tasks).
- Expected: times in one right-aligned column (the owner's rule: times and pills in fixed columns).
- Actual:
  - "Plain one 18:37" and "Notes one 20:34" end about 25 px left of the planned rows' "20:34" and of "28m" above.
  - In both rows the time box spans iOS x 316–373 (Android 286–343), with the text drawn at its left.
  - On Android, "1 queued" on running rows also ends 17 px left of their "28m".
- Shots: `e17-scheduled-android-360.png`, `d20-ios-390-light-bottom.png`.

## Notes (not counted)

- **Open in terminal app** was refused with "No terminal app on this computer for you here: open the Shell tab instead."
  - The cause is my env: `WISP_GRID_URL` is set, so `apps::in_wisp("auto")` counts the server as headless.
  - The ⋯ menu still offered the item, since `app` is computed without that check (`plugins/terminal/host/src/lib.rs:387` vs `:139`).
  - On a cloud box, the menu would offer an action that always refuses. The sentence also reads oddly ("on this computer for you here").
- **Panel toggle label:** the head button keeps "Show side panel (])" while the panel is open (`aria-pressed=true`).
- **Long-press:** on iOS and Android, CDP touch shows no change until the menu at about 505 ms. D measured iOS 5–37 ms the
  same way. CDP touch may not trigger `:active`, so this needs a device before it is called a regression.
- **Esc from a file view** in the panel closes the whole panel, not just the file.
- **Desktop Transcript:** Esc from the inline Transcript still goes Home (B, C, D note).
- **Phone crew stories strip** still stays fixed under the top bar, with rows cut under the avatars (D note, owner's call).
- **Grid at 390:** "Asleep · starts the fi…" and "No computers of …" are still cut (C, D note).
- **Forked review's Changes** lists the parent's edit under its absolute path `/private/tmp/qaE3/repos/app/math.js`.
  The fork's worktree was removed, and the edit was made in the parent's folder. A fork with its folder gone is rare, so
  this is not counted.
- **Not run:** `qa/parity.mjs` and `qa/layout.mjs`; agent sessions don't run the outer-loop gates.

## Cleanup

My two servers, both started by me, are stopped:
- `wisp serve` :4943;
- `wisp-grid dev` :4947, which stopped its boxes.

No other process was touched. Data stays under `/tmp/qaE3` and `/tmp/qa-keep/E`; shots are in `/tmp/qa-keep/shots-E`.
The shared `/tmp/wisp-target-fixer` was used as named and left in place.
