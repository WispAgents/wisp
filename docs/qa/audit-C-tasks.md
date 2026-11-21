# Audit C: tasks on desktop, phone and Grid web (wisp v2 @ 928fb5f3, 2026-11-19)

Findings only. No repo file was changed except this report.

**Setup.** This copies Audit B's setup (`docs/qa/audit-B-tasks.md`). Nothing real was touched.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-qaC3`. That dir started as an APFS copy-on-write clone of B's build dir, so the build took 3 minutes; it was deleted after the binaries were copied. The worktree is at HEAD 928fb5f3 plus other sessions' uncommitted edits under `design/v1-reference/`, which the build includes. The UI is a `vite build` in `/tmp/qaC3/web`.
- **Server:** `wisp serve --port 4967` with `WISP_HOME=/tmp/qaC3/wisphome` and `HOME=/tmp/qaC3/home`. Scripts: `/tmp/qaC3/serve.sh` and `env.sh`.
- **Fakes:** every harness (`WISP_*_BIN`) points at the fake agent. The `claude` wrapper replays a rich answer for prompts containing RICH and fails for FAILME. The `codex` wrapper reads its version from a file; editing the wrapper changes its fingerprint, which makes the changed-harness watch hold a turn. gh points at the fake gh.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`, and `WISP_GRID_URL` is the dev grid. The Grid runs blocked every non-local request, and none was attempted.
- **Grid:** `wisp-grid dev --port 4971 --data /tmp/qaC3/grid --wisp /tmp/qaC3/wisp`, signed in with dev handles `qc-one`, `qc-two`, `qc-three` and `qc-four`. Each got a cloud box that runs the same `wisp` binary and fakes.
- **Projects:** `app` (fake GitHub remote via `insteadOf`), `plain` (git, no remote), `notes` (no git).
- **Seeded tasks, every state (B's seed):**
  - review, done, failed (FAILME), archived, pinned, for later, unread;
  - running (gated `run1`, with 1 queued follow-up);
  - waiting with a question (`runtime.ask`, options);
  - held (the Codex wrapper changed while "Ask before running a changed harness" is on);
  - goal (`goal.set`);
  - compare group `g-qa1` (two `runtime.create` runs with `ext.wisp.runtime.group`, no worktree);
  - forked (`runtime.fork`);
  - crew (`crew.adopt` → qabot, `crew.create` → docbot);
  - scheduled (`later.add`);
  - rich blocks;
  - 14 fillers;
  - My Stuff: CSV, markdown, snippet and an app.
- **New in this seed:** a second compare group made the CLI way: `wisp "…" --agent claude --compare codex --cwd repos/app`. Both runs got their own worktrees (`wisp/task-…`), and the Codex run was held by the harness change.
  - Its titles start with "new " because I typed `wisp new "…"`. `new` isn't a verb, so it became part of the prompt; the help says `wisp "task"`. Not a finding.
- **Browsers:** Playwright from `/tmp/pw` with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop mode is stubbed with `window.__TAURI_INTERNALS__`, and Tauri calls are recorded.
  - Phones use the Capacitor stub (`isNativePlatform`, `getPlatform: 'ios' | 'android'`) at 390 and 360. Chrome uses CDP touch.
  - Light and dark.
- **Feedback** is the ms from pointerdown, touchstart or keydown to the first DOM mutation. Over 186 logged steps: median 7 ms, p95 31 ms. Full-page Grid navigations were timed by sampling the page at 40 and 90 ms.
- **Artifacts:**
  - Scripts: `/tmp/qa-keep/C/*.mjs` (B's scripts adapted, plus new probes).
  - Raw step logs: `/tmp/qa-keep/C/results-*.jsonl` (259 logged steps). Many logged FAILs were my own selectors. The list is now virtualized, so rows below the fold aren't in the DOM, and several phone controls moved. Each FAIL was re-checked by hand, and the tables below give the checked result.
  - Shots: `/tmp/qa-keep/shots-C/` (471 files).
- **Disk:** mid-run the disk filled up (ENOSPC). Other sessions' build dirs (`/tmp/wisp-target-*`, about 170 GB) took the space. I only deleted my own build dir; the run then went on.

## Counts

- **Action rows below: 92.** Many rows group several related actions, about 170 actions in all.
  - 81 pass.
  - 6 fail, one per finding (#1–#6).
  - 5 not tested, each with why.
- **Audit B's 10 findings, re-checked:**
  - 9 fixed: #1, #2, #3, #5, #6, #7, #8, #9, #10.
  - 1 still there, narrower: #4. WebKit's ResizeObserver loop is gone from the list, Home and My Stuff, but still fires on a task page with rich blocks.
- **NEW findings: 5.** 0 critical, 1 major, 4 minor (#1–#5 below).
  - Plus #6 below, B #4 still there (minor). It's counted in B's re-check, not as new.
- Console in Chrome: clean on every surface.
- Untranslated keys: none found (scan of rendered text for `plugin.key.path` strings).

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## Audit B's findings, re-checked

| B # | Finding | Now | How checked |
|---|---|---|---|
| 1 | Phone long-press runs the item under the lifted finger | **fixed** | C iOS 390/360, And 390/360, CDP touch: hold 0.7 s, lift. The menu or sheet stays open, and no command is sent (4 of 4). Shot `lp-android-released.png` |
| 2 | Ship on a project-checkout task fails with "no such session" | **fixed** | C D 1280: a `cwd` task's head has no Ship (deliberate, Ship only on worktree tasks). A worktree task's Ship opens "Ship it · wisp/task-684d15 · origin/main", fb 11 |
| 3 | Compare says "These runs are gone." | **fixed** | C D 1280: both groups (`g-qa1`, no worktree; the CLI `--compare` group) open the table, fb 6–10. Keep this asks first ("Keep Codex's result? The other run is archived…"), archives the other run, cleans its worktree, and toasts "Kept Codex's run. Ship it when you're ready." |
| 4 | WebKit "ResizeObserver loop completed with undelivered notifications" | **still there, narrower** (#6 below) | W D 1280 and W iOS 390: Home, list and My Stuff are clean (3 loads each). A task with rich blocks still throws it on 6 of 9 loads |
| 5 | Mark unread in the head jumps away with no message | **fixed** | C W D: toast "“Pinned one” marked unread · Undo". It still moves to the next task; Undo restores it and returns |
| 6 | Picking another harness changes earlier prompts' mark | **fixed** | C D 1280: after Codex → Default, the composer reads "Follow up with Codex…" and turn 1 keeps Claude Code's mark. Shot `reB-chromium-6-after.png` |
| 7 | Head Save for later has no Undo | **fixed** | C W D: "Saved for later · Undo" |
| 8 | iOS 360 compact bar runs the title into Search | **fixed** | C iOS 360 D, 390 L: "All projects ▾", Search, account, ⋯ and compose have room. Shot `reB2-8-ios-360-dark.png` |
| 9 | Live card status cut to "Claude Code is waiti…" | **fixed** | C And 390 D: "Claude Code is waiting for you · 8m 12s", and the action count dropped first. Shot `reB2-9-android-390-dark-waiting.png` |
| 10 | Grid Open on a sleeping box shows nothing for ~1.3 s | **fixed** | C 1280, fresh handle `qc-three` (asleep): Open carries `data-wake="Waking up · a few seconds…"`, and the box UI was up within 90 ms of the click. Account loads within 40 ms |

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows render, sections (Pinned, Needs you, Working, Scheduled, For later, Done, Archived) | pass | C D 1280 L, W D 1440 D. The list is virtualized now: lower sections render as you scroll |
| Fold / unfold each section | pass | C W; Needs you 16 → 4 rows, fb 3–14 |
| Open row (click) | pass | C W; fb 17 |
| Hover Save for later + Undo | pass | C W; fb 9 |
| Hover Mark done + Undo | pass | C W; fb 7 |
| Row ⋯ button opens the menu | pass | C W; fb 2 |
| Copy › flyout on hover (Copy task ID, Copy resume command) | pass | C W; clipboard and toast, fb 1–2 |
| Task options › flyout (Set goal, Side discussion, Add to crew, Save as recipe, Thread) | pass | C W; each opens its dialog or sheet, fb 1 |
| Pin / Unpin, Mark read / unread, Save for later + Undo | pass | C W; Pin moves the row to Pinned (`list-webkit-1440-dark-menu_Pin.png`) |
| Rename → Save | pass | C W; checked on a row in view (the script's row was below the fold) |
| Archive + Undo | pass | C W |
| Delete → Cancel / → Delete | pass | C W; asks first |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C W; navigates, opens a dialog, or `open_url vscode://file/…` |
| ⌘K → type → Enter | pass | C W; fb 7 |
| Project switcher (All ▾) → plain → ⌘0 | pass | C W; fb 6 |
| Hide / show the list ([) | pass | C W; fb 1 |
| J / K move through tasks | pass | C D; fb 2–4 |
| Compare group (both kinds) → vs Compare → table → Keep this | pass | C D 1280 (B #3 fixed) |
| Compare group card at narrow widths | **fail** | C iOS 360 D (#5) |
| Phone: rows, sections, crew stories | pass | C iOS/And 390, 360 L/D; W iOS 390 D |
| Phone: search → results | pass | C W iOS, C And 360; fb 6–9 |
| Phone: project title ▾ (large title and collapsed bar) → sheet | pass | C iOS 390/360, And 360; fb 8–10 |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C iOS 390 (bottom toolbar), And 360 (⋮ overflow): "Archived 2 tasks · Undo" restores both. W iOS selection bar shows |
| Phone: selection-mode row look | **fail** | C iOS 390 L, And 360 D (#4) |
| Phone: long-press → menu / sheet, stays open on lift | pass | C iOS/And 390/360 (B #1 fixed); W iOS (right-click fallback) |
| Phone: menu drill (Task options ›, and back) | pass | C iOS/And, W iOS; fb 2–6 |
| Phone: swipe right → Mark done + Undo | pass | C iOS 390, And 360; fb 31–34 |
| Phone: swipe left → Save for later + Undo | pass | C iOS 390, And 360 |
| Phone: pull to refresh | pass | C iOS 390, And 360; fb 32 |
| Phone: swipes and pull in WebKit | not tested | Playwright has no touch injection in WebKit |
| Phone: iOS compact bar at 360 | pass | B #8 fixed |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| Head: Changes chip → side Changes | pass | C W; fb 5 |
| Side panel tabs (Changes, Shell; phone sheet: Changes, PR, Shell, Preview, Thread) | pass | C W D; C iOS/And; fb 11 |
| Head Ship on a worktree task → "Ship it" dialog | pass | C; fb 11 |
| No Ship on a project-checkout task | pass | C W (deliberate) |
| ⋯ Transcript (V) | pass | C W; fb 6–11. Esc then goes Home (note) |
| ⋯ Open in › Open in Claude Code | pass | C W; terminal side opens, fb 3–5 |
| ⋯ Rename, Pin / Unpin | pass | C W |
| ⋯ Mark unread + Undo | pass | C W (B #5 fixed) |
| ⋯ Save for later + Undo | pass | C W (B #7 fixed) |
| ⋯ Fork…, Share…, Rerun… open | pass | C W; fb 11–16 |
| ⋯ Task options › Set goal, Add to crew, Save as recipe, Thread, Agent notes (radio) | pass | C W; fb 2–6 |
| ⋯ Copy › Copy task ID / resume command | pass | C W |
| Reply: type → Enter → new turn | pass | C D; C iOS/And (`composer.send`), fb 2–3 |
| Reply box + menu (Attach files, Goal…, Delegate…, Start later) | pass | C W; fb 1 |
| Start later → In 1 hour → "Starts at 07:03" chip → Send → `later.add` | pass | C W; the box clears, and the Scheduled section counts it. Enter instead of Send: #2 |
| Harness picker: Codex → "Follow up with Codex…"; turn marks kept | pass | C (B #6 fixed) |
| Mode picker (Plan / Edits / Full auto) | pass | C W; fb 1 |
| Running task: live card, Stop, "1 follow-up queued", Send now / Edit (inline edit, ↵ save · esc cancel) / Cancel | pass | C W D; C iOS/And |
| Waiting task: question card → option answers | pass | C D; C And D (answer fb 5, task runs on) |
| Held task: "Codex changed since wisp last ran it · Skip · Trust and start" → starts | pass | C D: `runtime.release`, the turn runs, fb 9, and the next Codex turn isn't held. Buttons 40–44 px tall on phones |
| Held task: another task held by the same change after Trust and start | **fail** | C D (#3) |
| Failed task: error and "Failed" pill | pass | C W |
| Rich answer (every block kind): layout | pass | C D 1440 D, W iOS 390 D |
| Rich answer: WebKit console | **fail** | W D 1280, W iOS 390 (#6, B #4) |
| Goal task: goal band ("Done when … · Working · Round 0") | pass | C D 1280 |
| Phone: back button and system Back | pass | C W iOS, C And; fb 5–10 |
| Phone: ⋯ sheet and its "This task ›" drill | pass | C iOS/And; fb 1–2 |
| Phone: live card header at 390 | pass | B #9 fixed |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 (it would post to the wisp team) |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects (app, notes, plain, No folder, Other folder…) → pick | pass | C W; fb 5–13. The 421 ms in the raw log was my script clicking the "in" label |
| Place picker (This computer · Folder / Worktree / In place) | pass | C W; fb 3–4 |
| Mode, harness, + menu (Attach, Start from an issue, Recipes, Goal, Delegate, Compare with another agent, Start later) | pass | C W; fb 1–22 |
| Launch with Enter → task page at once, in its own worktree | pass | C W; fb 2; cwd `wisphome/worktrees/app-…` |
| Start later on Home, then Enter | **fail** | C (#2) |
| Desktop drag strip: Home top → `start_drag`, double-click → `zoom_window` | pass | C W D |
| Desktop drag on the task head's middle (title) | pass | C W D |
| Phone New task: full-width composer, type → Start → task page | pass | C iOS 390, And 360, W iOS; fb 4–9 |
| N from Home focuses the launcher | pass | C D; fb 13 |

## My Stuff, crew, goals, later

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff: layout; Add files (file chooser) | pass | C W D |
| Open CSV viewer, markdown doc (desktop) | pass | C W D; fb 6–64 |
| Open CSV / markdown / code in the browser at 390 | pass | C web 390; the reader opens, fb 58 |
| Launch an app (iframe on its own port) | pass | C W D; C iOS/And (full-screen frame with Done), fb 70–71 |
| Card menu (Open, Use in a new task, Copy, Download, Star, Edit details, Show in folder, Remove) | pass | C W D |
| Remove + Undo | pass | C W D; C iOS/And ("Removed Snippet · Undo"), fb 1 |
| My Stuff search filters | pass | C D; "Counter" matched the app and its two uploaded copies' text (expected) |
| Phone Stuff: open a doc, CSV or code item (tap, or ⋯ → Open) | **fail** | C iOS 390, C And 360, W iOS 390 (#1) |
| Phone Stuff: row ⋯ (Open, Share, Copy, Download, Star, Remove) | pass | C iOS/And; fb 57–60 |
| Crew rail → member page; Org; Add to your crew | pass | C W D; fb 8–14 |
| Phone Crew tab (members, New report) | pass | C And 360 D, iOS 390 |
| Goal task: goal band | pass | C D |
| Scheduled task row and section | pass | C D (Scheduled 2 after the Start later probe) |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp / a real harness |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: box ⋯ (Copy address, Settings → Agents, API keys) | pass | C 1280, 390; fb 2–3 |
| Add ▾ (A computer of yours, Another cloud machine · Waitlist) | pass | C 1280, 390; fb 1–3 |
| Account page | pass | C; under 40 ms (full page) |
| Getting started → Continue | pass | C; opens the box (full page) |
| Open the cloud box (asleep) → wisp UI | pass | C 1280 (B #10 fixed) |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280; at 390 the box shows the phone list (FAB) |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost`; no hosts-file edits allowed |
| Shares from the Grid | not tested | no share on the dev grid |

## NEW findings

### 1. [major] Phone apps: opening a doc, CSV or code item in My Stuff leaves wisp for the raw file
- Engine: Chrome emulating the iOS and Android apps (Capacitor stub) at 390 and 360; WebKit iOS 390. Every try, 12 of 12.
- Steps: Stuff tab → tap "QA notes" (markdown), "Fruit table" (CSV) or "Snippet" (code). Or: the tile's ⋯ → Open.
- Expected: the item opens inside wisp, as it does on desktop and in a phone's browser at 390. There, wisp's reader shows "QA notes · notes.md · 1 min read" with the markdown rendered.
- Actual:
  - Nothing changes in the app: no DOM change at all within 1.5 s (fb null). ⋯ → Open only closes the menu.
  - A new window opens on `/api/stuff/<id>/file`, the raw file: "# QA notes · Some **markdown** here.", unrendered.
  - In a native shell, `window.open` may open the system browser, or nothing at all. On a phone reaching a remote computer, the relative file URL may not go through the computer's transport either.
  - Apps are fine: they open in-app, full screen with Done.
- Suspect:
  - `plugins/stuff/ui/src/StuffPhone.tsx:68` passes `launched` to `pageRun` but no `opened`.
  - So `plugins/stuff/ui/src/page-run.ts:59` falls back to `win()` (`open-here.ts:29`, `window.open(url, '_blank')`).
  - `page-run.ts`'s option doc (line 30) says the phone screen hands in `launched` and `opened`.
- Shots: `pstuff2-ios-QAnotes-touch.png` (after the tap: unchanged), `pstuff-popup-ios.png` (the raw file window), `pstuff3-chromium-web-tap.png` (the browser at 390: the reader).

### 2. [minor] After picking a Start later time, Enter reopens the + menu instead of sending
- Engine: Chrome, desktop 1280, in the task page's reply box and the Home launcher.
- Steps: type a reply → + → Start later › In 1 hour (the "Starts at 07:03" chip appears) → press Enter.
- Expected: Enter schedules it, as it sends a reply without the chip; or focus goes back to the text so Enter does.
- Actual:
  - Focus stays on the + button (`composer.plus` / `launcher.plus`), so Enter opens the + menu again (now without Start later).
  - Nothing is scheduled, and the text stays in the box. Clicking Send schedules it (`later.add`, the box clears).
- Shots: `later3-chromium-task-enter.png`, `later3-chromium-home-enter.png`.

### 3. [minor] Trust and start on one task leaves other tasks held by the same harness change
- Engine: Chrome, desktop 1280.
- Steps: the Codex wrapper changes; two Codex turns are held (the compare run and "Codex task"). On the compare run's card → Trust and start.
- Expected: the change is trusted for the harness, so every turn it held starts, as `accounts.watch.trust` does (`start_trusted`). At least, the other card stops saying the harness changed.
- Actual:
  - The compare run starts (`runtime.release`), and the trust re-pins: a new Codex turn on it runs without a hold.
  - "Codex task" stays Held in the list and on its page with "Codex changed since wisp last ran it · Skip · Trust and start", a change wisp now trusts.
- Shots: `held-cmpW2-after.png`, `keys-codex-held.png`.

### 4. [minor] Phone selection mode: the last row you tapped looks different from the other picked rows
- Engine: Chrome emulating iOS 390 light and Android 360 dark (CDP touch).
- Steps: Tasks → bar ⋯ → Select → tap two rows.
- Expected: both picked rows look the same: Android's selected tint, iOS's check alone.
- Actual:
  - The row tapped last keeps a grey rounded background (`:hover` and `:focus-within` are on it, though `(hover: hover)` is false).
  - On Android that grey replaces the selected tint, so one picked row is blue and the other grey.
  - On iOS only the last one is shaded.
  - Mobile browsers keep `:hover` on the tapped element after a tap, so this likely shows on devices too.
- Shots: `phone-chromium-android-360-dark-select.png` (Filler 6 blue, Filler 5 grey), `press-ios-390-light-selected.png`.

### 5. [minor] A compare group's card cuts its title to a few letters on a phone
- Engine: Chrome emulating iOS at 360, dark. Also desktop 1280 / 1440.
- Steps: Tasks with a compare group (two runs of one ask).
- Expected: the group's title keeps its words (design/layout.md: a row title is cut last). "vs Compare" and the check can shrink to their icons first.
- Actual:
  - iOS 360: the group header reads "TOPI… · Pick one", with "vs Compare" and a check icon in the middle.
  - Desktop: "new TOPIC[C… vs Compare", with free space left before the time.
  - The runs' own rows below show far more of the same title.
- Shots: `reB2-8-ios-360-dark.png` (header above Compare A / B), `look-task-1440-light.png` (desktop list).

## B #4, still there

### 6. [minor] WebKit still throws "ResizeObserver loop completed with undelivered notifications" on a task with rich blocks
- Engine: WebKit only. Desktop 1280 dark: 2 of 2 loads. iOS phone 390 dark: 2 of 3 loads. iOS 390 light: 2 of 2 loads. Chrome is clean.
- Steps: load `/task/<rich task>` (every block kind: code, checklist, assumption, Needs your call, Next actions, Follow up).
- Narrower than B: Home, the list, My Stuff and a task with a question card are clean (5 of 5 loads). The preload warnings B saw are gone.
- Expected: zero console errors.

## Notes (not counted)

- A waiting task's row reads "Starting up" (with the "Asking" pill), while its page says "Claude Code is waiting for you · Asked you: Which date library…". The seed asked through `runtime.ask` while the gated turn had printed nothing yet. A real agent asks through a tool call, which would give the row an activity line, so this may be the seed.
- Pressing and holding a phone row shows no pressed state until the menu opens at about 520 ms (no DOM change, nothing visible at 100 ms, iOS and Android). CDP touch may not trigger `:active`, so this needs a device to confirm.
- Desktop Esc from an inline Transcript still leaves the task for Home (B note).
- Mark unread in the head moves to the next task (now with a toast and Undo).
- The first Home load showed a "💻 62%" chip in the top bar while my build was running: the machine-load chip. It was gone once the build ended.
- `qa/parity.mjs` was not run (agent sessions may not run the outer-loop gates). The v1 differences seen by eye are listed as owner decisions in `docs/v2-plan.md` §3 and `design/v1-reference/allowed.md`: the head action bar, the one composer footer, and the project selector. So none is a finding here.
- Grid at 390: "Asleep · starts the first…" and "No computers of…" are cut in the machine rows. Left as is; the full text is one tap away.

## Cleanup

My two servers, both started by me, are stopped after this report:
- `wisp serve` :4967;
- `wisp-grid dev` :4971, which stops its boxes on exit.

No other process was touched. Data stays under `/tmp/qaC3` and `/tmp/qa-keep`. `/tmp/wisp-target-qaC3` is already deleted.
