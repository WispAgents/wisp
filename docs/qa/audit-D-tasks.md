# Audit D: tasks on desktop, phone and Grid web (wisp v2 @ b7598c9d, 2026-11-19)

Findings only. No repo file was changed except this report.

**Setup.** This copies Audit C's setup (`docs/qa/audit-C-tasks.md`). Nothing real was touched.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer` (incremental, 1 min 44 s). The worktree is at HEAD b7598c9d plus other sessions' uncommitted edits under `design/v1-reference/`. The UI is a `vite build` in `/tmp/qaD3/web`.
- **Server:** `wisp serve --port 4983` with `WISP_HOME=/tmp/qaD3/wisphome` and `HOME=/tmp/qaD3/home`. Scripts: `/tmp/qaD3/serve.sh` and `env.sh`. Ports 4900–4999 were all free when I started; I used 4983 and 4987.
- **Fakes:** as in C. Every harness (`WISP_*_BIN`) points at the fake agent. The `claude` wrapper replays a rich answer for RICH and fails for FAILME. Editing the `codex` wrapper changes its fingerprint, which makes the changed-harness watch hold a turn. gh points at the fake gh.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`, and `WISP_GRID_URL` is the dev grid. The Grid runs blocked every non-local request; none was attempted.
- **Grid:** `wisp-grid dev --port 4987 --data /tmp/qaD3/grid --wisp /tmp/qaD3/wisp`, signed in with dev handles `qd-one`, `qd-two` and `qd-three`. Each got a cloud box running the same `wisp` binary and fakes.
- **Projects:** `app` (fake GitHub remote via `insteadOf`), `plain` (git, no remote), `notes` (no git). All three were made fresh from C's files.
- **Seeded tasks:** C's seed scripts (`/tmp/qa-keep/D/seed*.mjs`), every state:
  - review, done, failed (FAILME), archived, pinned, for later, unread;
  - running (gated `run1`, with 1 queued follow-up);
  - waiting with a question (`runtime.ask`, options);
  - held (the Codex wrapper changed while "Ask before running a changed harness" is on);
  - goal (`goal.set`);
  - compare group `g-qa1` (two `runtime.create` runs with `ext.wisp.runtime.group`). Both runs now get their own worktree; in C they had none;
  - forked (`runtime.fork`);
  - crew (`crew.adopt` → qabot, `crew.create` → docbot);
  - scheduled (`later.add`);
  - rich blocks;
  - 14 fillers;
  - My Stuff: CSV, markdown, snippet and an app.
  - The CLI compare group: `wisp "…" --agent claude --compare codex --cwd repos/app`. Both runs got their own worktrees, and the Codex run was held by the harness change. So three Codex turns were held by one change: the CLI compare run, "Codex task", and a Codex follow-up on "Review me".
  - Added during the run: gated `run2` and `run3` (Send now and Stop probes), and `ask2` (a second question, for finding #1).
- **Browsers:** Playwright from `/tmp/pw` with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop mode is stubbed with `window.__TAURI_INTERNALS__`, and Tauri calls are recorded.
  - Phones use the Capacitor stub (`getPlatform: 'ios' | 'android'`) at 390 and 360. Chrome uses CDP touch.
  - Light and dark.
- **Feedback** is the ms from pointerdown, touchstart or keydown to the first DOM mutation. Over 206 logged steps: median 5 ms, p95 23 ms. Three were over 100 ms:
  - 409 and 410 ms: my script clicking the launcher's "in" label (as in C);
  - 504 ms: the Android long-press (see Notes).
- **Artifacts:**
  - Scripts: `/tmp/qa-keep/D/*.mjs`. These are C's scripts with paths and ports swapped, plus hand probes `d1.mjs`–`d23.mjs`.
  - Raw step logs: `/tmp/qa-keep/D/results-*.jsonl` (288 steps). As in C, many logged FAILs were my own selectors: the list is virtualized, and several controls only take pointer events after hover, which is what a person does. Every FAIL was re-checked by hand, and the tables give the checked result.
  - One more script artifact: my first list runs in Chrome and WebKit went at the same time on the same rows, so Chrome's hover-action Undo checks raced WebKit's. I count only the WebKit and serial results.
  - Shots: `/tmp/qa-keep/shots-D/` (about 510 files).

## Counts

- **Action rows below: 99.** Many rows group several related actions, about 185 actions in all.
  - 89 pass.
  - 5 fail, one per finding (#1–#5).
  - 5 not tested, each with why.
- **Audit C's 6 findings, re-checked: all 6 fixed** (#1, #2, #3, #4, #5, #6).
- **NEW findings: 5.** 0 critical, 1 major, 4 minor (#1–#5 below).
- Console: clean in Chrome and WebKit on every surface. This includes 9 WebKit loads of the rich-blocks task, where C #6 used to fire.
- Untranslated keys: none found (scan of rendered text for `plugin.key.path` strings).

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## Audit C's findings, re-checked

| C # | Finding | Now | How checked |
|---|---|---|---|
| 1 | Phone apps: opening a doc, CSV or code item in My Stuff leaves wisp for the raw file | **fixed** | C iOS 390 (QA notes, tap; Snippet, ⋯ → Open), C And 390 (Fruit table, QA notes, CDP tap): wisp's reader opens in the app, fb 16–70, no new window. Shots `pstuff2-ios-QAnotes-touch.png`, `pstuff2b-android-Fruittable-touch.png` |
| 2 | After picking a Start later time, Enter reopens the + menu | **fixed** | C D 1280: focus returns to the text box after the pick. In the task page (notes), Enter sends `later.add` (`later.list` has it, the box clears). On Home, Enter sends `later.plan` (`later.plans` has it). No menu reopens |
| 3 | Trust and start on one task leaves other tasks held by the same change | **fixed** | C D 1280: three Codex turns were held by one wrapper change (CLI compare run, "Codex task", "Review me"). Trust and start on the compare run (fb 9) released all three: each went held → done |
| 4 | Phone selection mode: the last row tapped looks different | **fixed** | C iOS 390 L and And 360 D, CDP touch: both picked rows look the same (iOS: the check alone; Android: the same tint on both). Shots `press-ios-390-light-selected.png`, `phone-chromium-android-360-dark-select.png` |
| 5 | A compare group's card cuts its title to a few letters on a phone | **fixed** | C iOS 360 D, And 360 D, D 1280/1440: the group's title has its own line ("TOPIC[CLI compare] …", "TOPIC[CLI compare] Make add faster" on desktop), with Pick one and vs Compare under it. Shot `d13-ios-360-dark-compare.png` |
| 6 | WebKit "ResizeObserver loop completed with undelivered notifications" on a task with rich blocks | **fixed** | W D 1280 D ×3, W iOS 390 D ×3, W iOS 390 L ×3: 0 console errors in 9 loads. A review task and a waiting task are clean too |

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows render, sections (Pinned, Needs you, Working, Scheduled, For later, Done, Archived) | pass | C D 1280 L, W D 1440 D. Virtualized: the lower sections render as you scroll (checked by scrolling to Archived) |
| Fold / unfold each section | pass | C W; Needs you 25 → 4 rows, fb 3–6 |
| Open row (click) | pass | C W; fb 16–22 |
| Hover Save for later + Undo | pass | W; fb 16. Chrome's run raced my parallel WebKit run on the same row, so it doesn't count; the menu's Save for later + Undo passed in Chrome |
| Hover Mark done + Undo | pass | W; fb 12, "Marked done · Undo" restores it |
| Row ⋯ opens the menu | pass | C W; fb 2–5 |
| Copy › flyout (Copy task ID, Copy resume command) | pass | C W; clipboard and toast, fb 1–3 |
| Task options › flyout (Set goal, Side discussion, Add to crew, Save as recipe, Thread) | pass | C W; each opens its dialog or sheet, fb 1–4 |
| Pin / Unpin | pass | C, by hand: Pinned 1 → 2, the row moves into Pinned, fb 9; Unpin puts it back |
| Mark read / unread | pass | C W; fb 3–4 |
| Rename → Save | pass | C, by hand: the field is focused with the title, Enter saves ("Filler 11 renamed") |
| Archive + Undo | pass | W; "Archived · Undo" restores it |
| Delete → Cancel / → Delete | pass | C W asks first; C Delete removes it (W's Delete step hit a row below the fold) |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C W; navigates, opens a dialog, or `open_url vscode://file/…` |
| ⌘K → type → Enter | pass | C W; fb 6–7 |
| Project switcher (All ▾) → plain → ⌘0 | pass | C W; fb 6–10 |
| Hide / show the list ([) | pass | C W; fb 0–1 |
| Archived section at the end of the list | pass | C, scrolled to the end: Working, Scheduled, For later, Done, Archived |
| Compare group → vs Compare → table → Keep this → confirm | pass | C D 1280: table fb 6; Keep this asks "Keep Claude Code's result? The other run is archived, and its worktree is deleted…", fb 2; confirm fb 23, toast "Kept Claude Code's run. Ship it when you're ready.", the Codex run archived |
| Compare group card on a phone | pass | C iOS 360 D, And 360 D (C #5 fixed) |
| Held row's status line | **fail** | C W D, C iOS/And (#2) |
| Phone: rows, sections, crew stories | pass | C iOS 390 L / 360 D, And 360 D, W iOS 390 D |
| Phone: search → results | pass | C W iOS; fb 5 |
| Phone: project title ▾ (iOS large title; Android title) → Projects sheet | pass | C iOS 390/360, W iOS 390, C And 360/390; fb 6–7 |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C W iOS 390 (bottom toolbar), fb 13–14 |
| Phone: Android selection bar (Mark done, Save for later, ⋮) | pass | C And 360 D: the contextual top bar shows; Archive sits in ⋮ |
| Phone: selection-mode row look | pass | C #4 fixed |
| Phone: long-press → menu / sheet, stays open on lift | pass | C iOS 390 (fb 5–37), And 360 (sheet at 504 ms, see Notes), W iOS (right-click fallback) |
| Phone: menu drill (Task options ›, and back) | pass | C iOS/And, W iOS; fb 2 |
| Phone: swipe right → Mark done + Undo | pass | C iOS 390 fb 33, And 360 fb 30; Undo sends the reverse `runtime.setState` |
| Phone: swipe left → Save for later + Undo | pass | C iOS 390 fb 33, And 360 fb 30 |
| Phone: pull to refresh | pass | C iOS 390 fb 28, And 360 fb 35 |
| Phone: Scheduled section, a scheduled new task's row | **fail** | C iOS 390 L, And 360 D (#4) |
| Phone: swipes and pull in WebKit | not tested | Playwright has no touch injection in WebKit |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| The turn's "1 file +1 −1" → side Changes | pass | C D; fb 5. The head no longer has a Changes chip; the turn link and the panel button (]) open Changes |
| Side panel (]) → Changes; tabs Changes / Shell | pass | C D; fb 5, tabs fb 1 |
| Head Ship on a worktree task → "Ship it" dialog | pass | C; "Ship it · wisp/task-c9bb57 · origin/main", fb 7 |
| No Ship on a project-checkout task | pass | C W (deliberate) |
| ⋯ Transcript (V) | pass | C W; fb 3–4. Esc then goes Home (as in C, note) |
| ⋯ Open in › Open in Claude Code | pass | C; the terminal side opens, fb 1 |
| ⋯ Rename, Pin / Unpin | pass | C W |
| ⋯ Mark unread + Undo | pass | C; toast "“Done one” marked unread · Undo", fb 9–12 |
| ⋯ Save for later + Undo | pass | C, by hand: "Saved for later · Undo", fb 7; Undo clears it, and the page stays |
| ⋯ Fork…, Share…, Rerun… open | pass | C W; fb 5–7 |
| ⋯ Task options › Set goal, Add to crew, Save as recipe, Thread, Agent notes (radio) | pass | C W; fb 1 |
| ⋯ Copy › Copy task ID / resume command | pass | C W |
| Reply: type → Enter → new turn | pass | C D (on Codex; held as expected after the harness change), C iOS/And phone reply fb 2 |
| Reply box + menu (Attach files, Goal…, Delegate…, Start later) | pass | C W; fb 2 |
| Start later → In 1 hour → Enter | pass | C D (C #2 fixed) |
| Harness picker: Codex → "Follow up with Codex…" | pass | C D; fb 8. The picker stays open after the harness pick for its model column; Esc or a click outside closes it |
| Mode picker (Plan / Edits / Full auto) | pass | C D; fb 1–2, `runtime.setMode` |
| Running task: live card, Stop, "1 follow-up queued" | pass | C D, C iOS/And |
| Queued follow-up: Edit (inline, Enter saves), Send now, Cancel | pass | C D, on hover (the buttons take pointer events once the row is hovered): Edit fb 15 and the edit is saved. Send now sends `runtime.queue.sendNow` (see Notes) |
| Stop a running turn | pass | `runtime.stop` on gated `run3`: stopped within 1 s; its row reads "You stopped this run… · Partial" |
| Waiting task: question card → option answers | pass | C D; fb 5, the task runs on |
| Held task: "Codex changed since wisp last ran it · Skip · Trust and start" → starts | pass | C D; fb 9, `runtime.release`. Phone buttons 44 px (iOS) and 40 px (Android) tall |
| Held task: every turn held by the same change starts | pass | C D (C #3 fixed) |
| Failed task: error and "Failed" pill | pass | C W |
| Rich answer (every block kind): layout | pass | C D 1280, W iOS 390 D |
| Rich answer: WebKit console | pass | C #6 fixed |
| Goal task: goal band ("Done when … · Working · Round 0") | pass | C W D, C iOS 390 L, And 360 D |
| Head details strip with the side panel open | **fail** | C D 1280 / 1440 (#3) |
| Phone: back button and system Back | pass | C W iOS, C And; fb 5–7 |
| Phone: ⋯ sheet and its "This task ›" / Show drills | pass | C iOS/And; fb 1–4 |
| Phone: the turn's files → Changes sheet (Changes, PR, Shell, Preview, Thread) | pass | C iOS 390, And 360; fb 50–55 |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 (it would post to the wisp team) |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects (app, notes, plain, No folder, Other folder…) → pick | pass | C, by hand: fb 6–7. The script's 410 ms was my click on the "in" label, as in C |
| Place picker (This computer · Folder / Worktree / In place) | pass | C W; fb 3 |
| Mode, harness, + menu (Attach, Start from an issue, Recipes, Goal, Delegate, Compare with another agent, Start later) | pass | C W; fb 1–24 |
| Launch with Enter → task page at once, in its own worktree | pass | C W; fb 2–3; cwd `wisphome/worktrees/app-…` |
| Start later on Home, then Enter | pass | C (C #2 fixed) |
| Needs you on Home (five shown) | **fail** | C D 1440 L, 1280 L; W D 1440 D (#1) |
| Top bar project switcher → Manage projects… | pass | C; switcher fb 4, Manage projects fb 9 ("/tmp/qaD3/repos/app · 37 tasks", Add, Browse…) |
| Desktop drag strip: Home top → `start_drag`, double-click → `zoom_window` | pass | C W D |
| Desktop drag on the task head's middle (title) | pass | C W D |
| Phone New task: full-width composer, type → Start → task page | pass | C iOS 390, And 360, W iOS; fb 4–16 |

## My Stuff, crew, goals, later

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff: layout; Add files (file chooser) | pass | C W D |
| Open CSV and markdown (desktop) | pass | C W D; fb 5–57 |
| Launch an app (iframe on its own port) | pass | C W D; fb 6–8 |
| Card menu (Open, Use in a new task, Copy, Download, Star, Edit details…, Show in folder, Remove) | pass | C W D; fb 3–4 |
| Remove + Undo | pass | C W D; "Removed “Snippet” from My Stuff · Undo" |
| My Stuff search filters | pass | C W D; "Counter" matches the app and its uploaded copies (as in C) |
| Phone Stuff: tabs, chips (All, Apps, Docs, Code), tiles | pass | C And 360 D, iOS 390 L |
| Phone Stuff: open a doc, CSV or code item | pass | C iOS 390, And 390 (C #1 fixed) |
| Phone Stuff: the reader's look on Android | **fail** | C And 390 L (#5) |
| Crew rail → member page; Org; Add to your crew | pass | C W D; fb 5–9 |
| Phone crew story: tap → full-screen story (now, last report, Good / Try again / Open report, next), Esc closes | pass | C And 360 D, iOS 390 D; fb 52–55 |
| Goal task: goal band | pass | C W D, phones |
| Scheduled section and rows (desktop) | pass | C D: Scheduled 3; a row opens its task, fb 19 |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp / a real harness (as in C) |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: box ⋯ (Copy address, Settings → Agents, API keys) | pass | C 1280, 390 (bottom sheet); fb 2 |
| Add ▾ (A computer of yours, Another cloud machine · Waitlist) | pass | C 1280, 390; fb 1 |
| Account page | pass | C 1280, 390 (a full-page load: the script's fb is null, as in C) |
| Getting started → Continue | pass | C 1280, 390; opens the box (a full-page load) |
| Open the cloud box (asleep) → wisp UI | pass | C 1280, fresh handle `qd-three`: Open carries `data-wake="Waking up · a few seconds…"`, and the box UI was up within 90 ms of the click |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280; at 390 the box shows the phone list (FAB), as in C |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost`, and hosts-file edits aren't allowed (as in C) |
| Shares from the Grid | not tested | no share on the dev grid (as in C) |

## NEW findings

### 1. [major] Home's Needs you leaves out the task that is asking you a question
- Engine: Chrome desktop 1440 L and 1280 L; WebKit desktop 1440 D. 3 of 3 loads.
- Steps: a task asks a question (`runtime.ask`) while five or more newer tasks need you (finished in review, or held). Open Home.
- Expected: the asking task is in Home's five, first, as in the sidebar's Needs you. v1's Home was "the sidebar's Needs you" (`web/js/features/split.js` `paintHome` takes `groups().needs`), and `groups()` sorts waiting tasks first, then newest (`web/js/features/feed.js:96`).
- Actual:
  - The sidebar puts "gate ask2 TOPIC[Asks again]… · Asking" first under Needs you 30.
  - Home's "Needs you 5 of 32" shows Filler 6, 5, 4, 3 and 2, all just Finished, and not the task waiting on an answer.
  - Home is where you look first, and the asking task is the one blocked on you.
- Suspect: `plugins/shell-home/ui/src/needs.ts:18` sorts by `updatedAt` only (`b.updatedAt - a.updatedAt`), without v1's waiting-first key.
- Shots: `d23-home-needs-asking-left-out.png` (sidebar and Home side by side), `d1-pin.png` (the same with the first asking task).

### 2. [minor] A held row says "Waiting for codex", the harness's raw id
- Engine: Chrome and WebKit, desktop 1280/1440, iOS 390, Android 360. Every held row: the CLI compare run, "Codex task", and "Review me" after its Codex follow-up.
- Steps: a Codex turn is held by the changed-harness watch. Look at its row in the list.
- Expected: "Waiting for Codex", the name every other place uses (the row's own third line says "Codex · This computer"; the held card says "Codex changed since wisp last ran it").
- Actual: "Waiting for codex", lowercase, on the list rows on every surface.
- Suspect: `plugins/shell-list/ui/src/model/row.ts:115` builds it with `agentName(s.agent, agents)`, which falls back to the id when the harness isn't found in the `agents` it is handed.
- Shots: `d1-pin.png`, `phone-chromium-android-360-dark-home_layout.png`, `press-ios-390-light-selected.png`.

### 3. [minor] With the side panel open, the task head's details strip cuts the harness name while the branch keeps empty room
- Engine: Chrome desktop 1280 and 1440, light. Side panel open (`?side=changes`) on a worktree task.
- Steps: open a worktree task (Compare A) and its side panel (]).
- Expected: the strip under the title reads "Claude Code · Default model · app · wisp/task-c9bb57", or cuts last what matters least. design/layout.md: no cut text while there is room.
- Actual:
  - 1280: "Claude Code…" (the model gone), then about 140 px of empty space after the branch.
  - 1440: "Claude Code · Default mo…".
  - `split.details`: the branch item's box is 260 px wide (`scrollWidth` 260) for about 120 px of text, so the harness item gets 160 px (1280) or 236 px (1440).
- Shots: `d4-meta-1280.png`, `d4-meta-1440.png`, `d2-chromium-side.png`.

### 4. [minor] Phone: a scheduled new task's row in Scheduled has its title out of line with the task rows
- Engine: Chrome emulating iOS 390 light and Android 360 dark.
- Steps: schedule a new task from Home (+ → Start later → In 1 hour → Enter). Scroll the Tasks list to Scheduled.
- Expected: its title starts where every row's title starts (x = 47 on both phones), with the clock in the dot column, as on desktop (101 vs 100).
- Actual:
  - The planned task's title (`list.plan.title`) starts at x = 68 on iOS 390 and x = 72 on Android 360, 21–25 px right of "Notes one" and "Plain one" above and below it.
  - Its two lines ("Scheduled task", "app") make it taller than the rows around it.
- Shots: `d20-ios-390-light-bottom.png`, `d21-desktop-light-scheduled.png` (desktop, aligned).

### 5. [minor] Android: My Stuff's reader opens as a tinted card with the desktop window's header inset in it
- Engine: Chrome emulating Android at 390, light. Settled after 3.5 s.
- Steps: Stuff tab → tap "QA notes" or "Fruit table".
- Expected: the reader as a full-screen page, as on iOS (white page, header bar across the top), or a Material 3 sheet with its own top bar.
- Actual:
  - The reader sits on a light-blue sheet that stops about 50 px above the bottom with rounded bottom corners, so the tab bar shows under it.
  - Inside, the desktop reader's header (file icon, title, "notes.md · 1 min read", Copy, ×, blue rule) is a grey box inset 24 px from the sheet's edges.
  - Two surfaces show at once; iOS shows one clean page.
- Shots: `pstuff2b-android-QAnotes-touch.png`, `pstuff2b-android-Fruittable-touch.png`, `pstuff2-ios-QAnotes-touch.png` (iOS, for comparison).

## Notes (not counted)

- **Send now on a running turn:** it sends `runtime.queue.sendNow`, which answers ok, but nothing visible changes.
  - The gated fake hadn't sent a session id yet, so the run can't be resumed. `queue::send_now` then moves the item to the front (`SendNow::Next`) instead of interrupting.
  - A real agent sends its session id at once, so this is the seed. Still, the press shows nothing; a "next" mark could say so.
- **Long-press on Android:** pressing and holding a row shows no pressed state until the sheet opens at about 504 ms (no DOM change by 100 ms). Same as C; CDP touch may not trigger `:active`, so this needs a device.
- **Desktop Transcript:** Esc from an inline Transcript still leaves the task for Home (B, C note).
- **Phone crew stories strip:** it stays fixed under the top bar while the rows scroll under it. A row is cut mid-line under the avatars, with no edge (`d13-ios-360-dark-compare.png`, "Review me" sliced). The owner's design (`docs/v2-plan.md` 13b, direction C) puts the strip under the top bar but doesn't say whether it scrolls away, as in Instagram. Left as the owner's call.
- **Phone compare card:** the card's border runs to within 1 px of both screen edges (360 of 360), while the rest of the list has no card. It reads as a full-bleed card with rounded corners. Look only.
- **"1 file +1 −1":** each fake turn reports one edit, but the worktree has no diff, so Changes says "No changes yet". The fake reports the edit, so this is the seed.
- **Needs you counts:** the sidebar says 30 while Home says "5 of 32". v1's Needs you count is deliberate (per the brief), so not a finding. The per-project "need you" counts were checked: notes and plain show none because each has a follow-up scheduled (one by my own C #2 probe), so both are in Scheduled.
- **Not run:** `qa/parity.mjs` (agent sessions may not run the outer-loop gates). The v1 differences seen by eye are listed as owner decisions in `docs/v2-plan.md` §3 and `design/v1-reference/allowed.md`: the head action bar, the one composer footer, the project selector and the phone's native design. So none is a finding here.
- **Grid at 390:** "Asleep · starts the first…" and "No computers of…" are still cut in the machine rows (C note).

## Cleanup

My two servers, both started by me, are stopped after this report:
- `wisp serve` :4983;
- `wisp-grid dev` :4987, which stops its boxes on exit.

No other process was touched. Data stays under `/tmp/qaD3` and `/tmp/qa-keep`. The shared `/tmp/wisp-target-fixer` was used as named and left in place.
