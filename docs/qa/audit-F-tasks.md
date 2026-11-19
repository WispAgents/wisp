# Audit F: tasks on desktop, phone and Grid web, with the task side panel (wisp v2 @ d4c98dac, 2026-11-19)

Findings only. No repo file was changed except this report.

**Setup.** This copies Audit E's setup (`docs/qa/audit-E-tasks.md`) and the side panel audit's states
(`docs/qa/audit-side-panel.md`), in a fresh `/tmp/qaF3`. Nothing real was touched: no real harness, no service, never
ports 4477, 4800, 4791 or 4329, and never `~/.wisp*`.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`
  (incremental, 1 m 50 s, queued behind another session's `wisp-grid` build on the same lock). HEAD d4c98dac, plus other
  sessions' uncommitted edits under `design/v1-reference/` (no source file differs from HEAD). The UI is a `vite build`
  in `/tmp/qaF3/web`.
- **Server:** `wisp serve --port 4951` with `WISP_HOME=/tmp/qaF3/wisphome` and `HOME=/tmp/qaF3/home`. Scripts:
  `/tmp/qaF3/serve.sh` and `env.sh`. Nothing in 4900–4999 was busy when I started; I used 4951 and 4957.
- **Fakes:** as in E. Every harness points at the fake agent. The `claude` wrapper replays a rich answer for RICH and fails
  for FAILME. Appending a line to the `codex` wrapper changed its fingerprint, which made the changed-harness watch hold
  turns. gh points at the fake gh.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`, and `WISP_GRID_URL` is the dev grid.
- **Grid:** `wisp-grid dev --port 4957 --data /tmp/qaF3/grid --wisp /tmp/qaF3/wisp`, dev handles `qf-one`, `qf-two` and
  `qf-three`.
- **Projects:** `app` (fake GitHub remote via `insteadOf`), `plain` (git, no remote), `notes` (no git), made fresh from E's
  first commits.
- **Seeded tasks:** E's seed scripts with paths, ports and ids swapped (`/tmp/qa-keep/F/seed1-3.mjs`, `seedE.mjs`,
  `seedstuff.mjs`), plus `seedF.mjs` (two planned new tasks, through `later.plan` with `kind: task`), `seedheld.mjs` and
  `seedwt.mjs`. Every state:
  - review, done, failed (FAILME), archived, pinned, for later, unread;
  - running (gated `run1`, `run2`, `run3`; `run1` and `run2` with a queued follow-up);
  - two asking questions (`runtime.ask` on gated `ask1`, `ask2`);
  - held: two Codex turns ("Codex task", Compare B) held by one wrapper change;
  - goal, forked, crew (qabot adopted, docbot created), scheduled (`later.add`, two `later.plan`), rich blocks, 14 fillers;
  - My Stuff: CSV, markdown, snippet and an app;
  - **compare group** `g-qa1` (Compare A on Claude Code, Compare B on Codex), plus the CLI compare
    `wisp "TOPIC[CLI compare] Make add faster" --agent claude --compare codex`. Every compare run got its own worktree.
    Group runs keep their prompt as the title (`finish.rs` `title_auto` skips groups), so their rows read
    "TOPIC[Compare A] …".
  - **Worktree one**: a plain task in its own worktree (`worktree: true`), for Mark done, Stage, Revert and Commit.
- **Side panel states** (as in the side panel audit and E):
  - a worktree with real edits (Compare A: `math.js` changed, `extra.txt` added by hand);
  - a task in place while the project checkout is dirty (Review me; `README.md` edited by "the user");
  - a task whose worktree was removed (Forked review; `git worktree remove`);
  - a folder without git (Notes one);
  - an open PR: Ship → Commit & open PR on the CLI compare's Claude run gave PR #1 on the fake gh;
  - a My Stuff app pinned beside tasks (Counter app, Pin beside tasks);
  - the Thread tab.
- **Browsers:** Playwright from `/tmp/pw`, with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop is stubbed with `__TAURI_INTERNALS__`. `window.open` is recorded.
  - Phones use the Capacitor stub (`ios` / `android`) at 390 and 360. Chrome uses CDP touch.
  - Light and dark.
  - Every browser is closed in a `finally` (or the exit handler in `lib.mjs`).
- **Feedback** is the ms from pointerdown, touchstart or keydown to the first DOM mutation. Over 195 logged steps with a
  reading: median 6 ms, p95 32 ms. Over 100 ms, the same as E:
  - 409 and 506 ms: my script clicking the launcher's "in" label;
  - 505 ms: long-press on iOS and Android (see Notes).
- **Artifacts:**
  - Scripts: `/tmp/qa-keep/F/*.mjs`. These are E's scripts with paths, ports and ids swapped, plus hand probes
    `f1.mjs`–`f15.mjs`.
  - Raw step outputs: `/tmp/qa-keep/F/*.out` and `results-*.jsonl`.
  - As in C–E, many scripted FAILs were my own selectors: the list is virtualized, rows near the bottom sit under the
    phone FAB, the iOS script tapped the collapsed title, and titles with `[` broke my regexes. Every FAIL was re-checked
    by hand, and the tables give the checked result.
  - Seed noise from my own scripts: E's `e4.mjs` typed `echo hello-qa` while the focus was on the Shell tab (NEW #1),
    which marked Compare A done and queued "ho hello-qa" on Compare B. I put Compare A back to review and cancelled the
    queued item. "Launched B" (twice) and "Phone3 android 360" are the phone scripts' own launches.
- **Shots:** `/tmp/qa-keep/shots-F/` (534 files).

## Counts

- **Action rows below: 114.** Many rows group several related actions, about 220 actions in all.
  - 103 pass.
  - 6 fail: one for each NEW finding (#1–#5) and Audit E #2, which still fails.
  - 5 not tested, each with why.
- The re-check tables (Audit E's 11, Audit D's #1, the side panel audit's 12) come on top of these rows.
- **Audit E's 11 findings, re-checked:** 10 fixed; 1 still fails (#2, the phone sheet strip at 360 with a pinned tab).
- **Audit D #1 (Home's Needs you leaves out the asking task):** fixed.
- **Side panel audit's 12 findings, re-checked:** all 12 hold.
- **NEW findings: 5.** 0 critical, 2 major, 3 minor (#1–#5 below).
- Console: clean in Chrome and WebKit on every surface, except one WebKit run that logged "preloaded but not used"
  warnings (see Notes). Untranslated keys: none.

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## Audit E's findings and D #1, re-checked

| # | Finding | Now | How checked |
|---|---|---|---|
| D 1 | Home's Needs you leaves out the task asking you a question | **fixed** | C D 1440 L, 1280 L; W D 1440 D: with ask1 and ask2 asking, Home's "5 of 29" starts with both ("Asking"), as the sidebar does. Shot `e1-home-chromium-1440-light.png` |
| E 1 | After Open file… picks a file, the focus is left on the page | **fixed** | C W D 1440: after the pick the focus is the file's editor (`TEXTAREA` "math.js: edit, then Save (⌘S)"). No shortcut runs |
| E 2 | Phone side sheet: with a pinned tab, the strip runs past the screen at 360 | **still fails** | C iOS 360 D: the tab list keeps x 16–377 (361 px) on a 360 px screen, Thread reads "Th". C And 360 D: x 6–392, Thread is outside the screen. `scrollWidth == clientWidth` (no scroll). Shots `e29-phone-sheet-ios-360-dark.png`, `e29-phone-sheet-android-360-dark.png` |
| E 3 | Phone Changes: file names cut to two letters | **fixed** | C iOS 360/390, And 360/390: "extra.txt" and "math.js" whole on their own line, Stage · Viewed · ✎ · ⋯ and the counts on the line below. Same shots |
| E 4 | Changes on a worktree says "edits wisp saw" over git's diff | **fixed** | C W D 1440, read after 6 s: "vs origin/main", title "Compared with origin/main where this branch left it, uncommitted changes included" |
| E 5 | `→` drops focus to the page at Thread or a pinned tab | **fixed** | C W D 1440: Changes → PR → Shell → Preview → Thread → Changes, the focus on each tab (fb 3–21) |
| E 6 | At 1280, opening a pinned tab turns the panel's tabs into icons while there's room | **fixed** | C W D 1280 (297 px): with Counter app open, "[Changes] PR Shell [Preview] Counter app [Thread]" uses 296 of 297 px. PR and Shell keep their labels; Changes and Preview give way because the row can't hold more. Closed, "Changes 2 PR Shell [Preview] [Counter] [Thread]". As icons, Changes shows no count (its label has it). Shots `e16-pinned-open-1280.png`, `e34-webkit-pinned-open-1280.png` |
| E 7 | Copy path and Copy branch name in the panel's ⋯ say nothing | **fixed** | C W D: the ⋯ turns into a check, labelled "Copied", for a moment; no toast, on purpose (`SidePanel.tsx:95`, the owner's "toasts are for errors"). Clipboard right. Shot `f3-chromium-Copy_path-60ms.png` |
| E 8 | The PR tab reads "#1" without "PR" | **fixed** | C W D 1280/1440: "PR #1" (61 / 70 px). At 1280 Preview and Thread give way to it |
| E 9 | Undo after Mark done leaves "Remove its worktree?" open | **fixed** | C W D 1280, on Worktree one: Undo puts it back to review and the dialog goes; the worktree stays |
| E 10 | Changes that wisp saw show "⋯ 0 unchanged lines" | **fixed** | C W D 1440: Review me's two hunks run on with no fold row |
| E 11 | Phone: a task row's clock time in Scheduled sits 25 px left | **fixed** | C iOS 390: every time ends at x 373–374; And 360: 343–344. Desktop 372–373 |

## Side panel audit's findings, re-checked

| SP # | Finding | Now | How checked |
|---|---|---|---|
| 1 | The strip hid v1's tabs behind `+` | **holds** | C W D 1280/1440: Changes, PR, Shell, Preview, then pinned and Thread; no `+` |
| 2 | Labels hidden under 420 px | **holds** | C W D 1280: Changes 2, PR, Shell, Preview labelled, Thread its icon (303 px row); 1440 the same with more room |
| 3 | A pinned tab's label squeezed the panel's own | **holds** | Closed, the pinned tab goes to its icon first (E #6 above) |
| 4 | `→` moved one tab, then focus fell to the page | **holds** | E #5 above |
| 5 | Background reads toasted raw errors | **holds** | C W D: no toast on the worktree-gone, in-place and no-git tasks |
| 6 | Changes on a task in place diffed the whole checkout | **holds** | C W D: Review me lists only math.js (edits wisp saw), not the user's `README.md` edit |
| 7 | Changes on a task whose worktree is gone said "No changes yet" | **holds** | C W D: Forked review shows the edits wisp saw, no toast |
| 8 | Open file… on a folder git can't list | **holds** | C W D: not offered on the in-place, gone and no-git tasks |
| 9 | End shell ended at once | **holds** | C W D: "End the shell? Whatever runs in it stops with it. · Cancel · End shell", fb 0–1. Cancel keeps it; End → "Shell ended · Start again" |
| 10 | A reattached shell kept its old size | **holds** | C W D: a shell started at 1440 refits to the 395 px pane at 1280; nothing past the edge |
| 11 | The Shell in a dark box inside a white margin | **holds** | C W D: the terminal fills the pane (x 961–1440), `zsh · End shell` foot on its colours. Shot `f4-shell-chromium-light.png` |
| 12 | Head Changes chip on a task in place showed the checkout's diff | **holds** | C W D: no chip on in-place, gone and no-git tasks. On Compare A "+3 −1" matches the panel |

## Side panel

| Action | Result | Where / how checked |
|---|---|---|
| `]` opens, `]` and × close | pass | C W D; fb 5–10 |
| Head button toggles the panel | pass | C W D; fb 5–10. Its label stays "Show side panel (])" while open (Notes) |
| Strip at 1280 / 1440, no pins | pass | C W D L/D (SP #2) |
| Strip with a PR and a count | pass | C W D: 1280 "Changes 1 · PR #1 · Shell · [Preview] · [Thread]" (274 px); 1440 "Changes 1 · PR #1 · Shell · Preview · [Thread]" |
| Strip with a pinned app, closed / open | pass | E #6 |
| Each tab opens (Changes, PR, Shell, Preview, Counter app, Thread) | pass | C W D; fb 0–1 |
| `←` / `→` walk the tabs | pass | E #5 |
| Shell tab takes the keys | **fail** | C W D 1440 (#1) |
| Changes on a worktree: list, fold a diff, Collapse all | pass | C W D; fold fb 1 |
| Changes header words on a git diff | pass | E #4 |
| Changes on a task in place / worktree gone / no git | pass | SP #6, #7 |
| Viewed | pass | C D: fb 1, "1 of 3 viewed", the file folds |
| Stage a file | pass | C D: fb 4, `git.stage`, git shows `M  README.md` |
| Revert a hunk; ⋯ Revert file | **fail** | C W D (#2) |
| Commit from Changes | **fail** | C D: commits ("Committed 69d0e99"), but the list then drops the committed files (#3) |
| Open file… → pick a file | pass | E #1 |
| `⋯` lists Copy path, Copy branch name, Open in editor, Open in terminal app | pass | C W D |
| Copy path, Copy branch name | pass | E #7 |
| Open in editor | pass | C D: opens in the click |
| Open in terminal app | not tested | Refused, "No terminal app on this computer for you here…": my env sets `WISP_GRID_URL`, so the server counts itself headless (E Notes) |
| PR: card, checks, Merge asks first (cancelled) | pass | C W D 1280/1440: "feat: fake claude finished · #1 · Open · Ready to merge · Merge · Checks · Open on GitHub". Merge asks "Merge this pull request? It squash-merges…", fb 2; Esc cancels |
| PR: Merge → confirm | pass | C D 1440: fb 3, `ship.merge`; "#1 · Merged · Merged into main"; local main has the squash commit; the worktree is removed |
| PR tab label | pass | E #8 |
| Ship → Commit & open PR | pass | C D: fb 10, "Opened PR #1", `ship.ship`. Before the edit, Ship said "Nothing to ship yet" with Commit & open PR off |
| Shell: start, type (after a click into it), End shell asks, Start again | pass | SP #9; `echo f4-qa-$((6*7))` → `f4-qa-42` |
| Shell reattached at another width | pass | SP #10 |
| Preview: says how to run; Run offered | pass | C W D: "Run the app from this task's folder and see it here · Runs with · Run" |
| Pinned My Stuff tab opens its app | pass | C W D: "Counter app", fb 1 |
| Thread tab | pass | C W D: Notes headings (Goal, Constraints, Decisions…) |
| Esc with the panel open | pass | C D: Esc closes the panel. From a file view the first Esc only leaves the editor (Notes) |
| Resize by the grip (1440) | pass | C D (as E) |
| Phone sheet: tabs, Changes, PR | pass | C iOS 390 L, And 390 L: Changes 2, PR, Shell, Preview, Counter app, Thread; fb 7 |
| Phone sheet strip at 360 with a pinned tab | **fail** | E #2 |
| Phone Changes file rows on a worktree | pass | E #3 |

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows render; sections Pinned, Needs you, Working, Scheduled, For later, Archived | pass | C D 1280 L, W D 1440 D; at the end: Working 4, Scheduled 3, For later 1, Archived 1 |
| Fold / unfold a section | pass | C W; Needs you 27: 13 → 6 rows, fb 3 |
| Open a row (click) | pass | C W, by hand; fb 17–19 |
| Hover Save for later + Undo | pass | C W, by hand; fb 6–12 |
| Hover Mark done + Undo | pass | C W, by hand; fb 4–8 |
| Mark done on a worktree task: "Remove its worktree?", then Undo | pass | E #9 |
| Row ⋯ menu | pass | C W; fb 2 |
| Copy › (task ID, resume command) | pass | C; toast, clipboard |
| Task options › Set goal, Side discussion, Add to crew, Save as recipe, Thread | pass | C; fb 1–2 |
| Pin / Unpin | pass | C W, by hand: Pinned 2 → 3, the row at the top |
| Mark read / unread, Save for later + Undo, Archive + Undo | pass | C; fb 7–10 |
| Rename → Save | pass | C ("renamed") |
| Delete → Cancel / → Delete | pass | C (Filler 13 gone) |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C; navigates or opens a dialog |
| ⌘K → type → Enter | pass | C W; fb 6 |
| Project switcher (All ▾) → plain → ⌘0 | pass | C W; fb 6 |
| Hide / show the list ([) | pass | C W; fb 0 |
| Compare group → vs Compare → table → Keep this → confirm | pass | C D 1280 (as E's e33 flow) |
| Compare group card: edges and the right column | **fail** | C W, phones and desktop (#5) |
| Held rows | pass | C W D: "Codex task · Waiting for Codex · Held", Compare B "Held · 1 queued" |
| Asking rows first | pass | C W D: ask2 and ask1 under Pinned, "Asking" |
| Phone: rows, sections, crew story, bottom tabs | pass | C iOS 390 L, And 360 D, W iOS 390 D; tabs fb 4–6 |
| Phone: search → results | pass | C W iOS; fb 6–8 |
| Phone: project title ▾ → Projects sheet | pass | C And 360 (fb 5–6), C iOS by hand; the scripted FAIL tapped the hidden small title |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C And 360: "Archived 2 tasks · Undo", both back. W iOS 390: bottom toolbar |
| Phone: long-press → sheet / menu, drill into Task options | pass | C iOS 390, And 360; menu at 505 ms, drill fb 2 |
| Phone: swipe right → Mark done + Undo | pass | C iOS 390 fb 33, And 360 fb 28 |
| Phone: swipe left → Save for later + Undo | pass | C iOS 390 fb 30, And 360 fb 33 |
| Phone: pull to refresh | pass | C iOS fb 32, And fb 32–34 |
| Phone: Scheduled section times | pass | E #11 |
| Phone: swipes and pull in WebKit | not tested | no touch injection in WebKit |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| Head: Ship on a worktree task → "Ship it" dialog | pass | C; fb 7–10 |
| No Ship on a project-checkout task | pass | C W (deliberate) |
| ⋯ menu (Transcript, Shell, Rerun, Fork, Open in, Rename, Pin, Mark unread, Save for later, Task options, Copy, Share, Archive, Delete) | pass | C W; fb 1–4 |
| ⋯ Save for later + Undo | pass | C, by hand: "Saved for later · Undo", fb 9, the page stays (the scripted "url moved" was an earlier step's) |
| ⋯ Mark unread + Undo, Pin / Unpin | pass | C W |
| ⋯ Transcript (V) | pass | C W; fb 4–7. Esc then goes Home (note, as in B–E) |
| ⋯ Open in › Open in Claude Code | pass | C W; fb 1, the terminal tab opens |
| Reply: type → Enter → new turn | pass | C D; C iOS/And fb 2 |
| Reply + menu, mode picker, harness picker | pass | C W; fb 1–8 |
| Start later → time → Enter | pass | C W: focus returns to the box (the script didn't press Enter, as in E) |
| Queued follow-up shown, Send now / Edit / Cancel | pass | C D, by hand on Compare B's queue: `runtime.queue.cancel` |
| Stop a running turn | pass | C D (as E) |
| Waiting task: question card → answer | pass | C D (ask1 "You answered: dayjs"); C And 390 D and W iOS 390 L: "Needs you · Ship to staging or prod? · staging · prod", tap fb 5–7, the turn runs on |
| Held task: "Codex changed since wisp last ran it · Skip this run · Trust and start" | pass | C D 1280 |
| Held card on phones | **fail** | C iOS 390, And 360 (#4) |
| Failed task, rich blocks, goal band | pass | C W D, phones |
| Head details strip with the side panel open | pass | C W D 1280/1440: "Claude Code · Default model · app · wisp/task-c0aafa", nothing cut |
| Phone: back button and system Back | pass | C W iOS, C And; fb 5–9 |
| Phone: ⋯ sheet and its drills | pass | C iOS/And; fb 1–3 |
| Phone: reply box full width + send | pass | C iOS (24 to 366), And (24 to 336) |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects → pick | pass | C (the scripted 409/506 ms was my click on "in", as in C–E) |
| Place, mode, harness, + menu | pass | C W; fb 1–9 |
| Launch with Enter → task page at once, in its own worktree | pass | C W; fb 2 |
| Needs you on Home | pass | D #1 fixed |
| Top bar switcher → Manage projects… | pass | C W |
| Desktop drag strip: Home top, task head middle | pass | C W D: `start_drag`, double-click `zoom_window` |
| Phone New task → Start → task page | pass | C iOS 390, And 360; fb 4 |
| Phone New task composer full width | pass | C iOS 8–382, And 8–352; fb 14–20 |

## My Stuff, crew, goals, later

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff layout; Add files | pass | C W D |
| Open CSV and markdown | pass | C W D |
| Launch an app | pass | C W D |
| Card menu (Launch, Pin beside tasks, Pin on home, Use in a new task, Star, Edit details…, Show in folder, Remove) | pass | C D; fb 3 |
| Pin beside tasks | pass | C D: "Pinned Counter app beside tasks · Undo", fb 5; it shows in the panel strip |
| Remove + Undo | pass | C W D |
| Search filters | pass | C W D; "Counter" matches the app and its copies (as in C–E) |
| Phone Stuff: tabs, open a doc | pass | C iOS/And |
| Crew rail → member page; Org; Add to your crew | pass | C W D |
| Goal task: goal band | pass | C W D, phones |
| Scheduled section (desktop): rows open their task | pass | C D: Scheduled 3 (Plain one's follow-up and two planned tasks) |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp or a real harness (as in C–E) |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Landing and dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: box ⋯ (Copy address, Settings → Agents, API keys; Sleep now once up) | pass | C 1280, 390; fb 2 |
| Add ▾ (A computer of yours, Another cloud machine · Waitlist) | pass | C 1280, 390; fb 1 |
| Account page | pass | C 1280, 390 (a full-page load; the script's fb is null, as in D, E) |
| Getting started → Continue | pass | C 1280, 390; opens the box (full-page load) |
| Open the cloud box (asleep) → wisp UI | pass | C 1280, fresh `qf-three`: Open carries `data-wake="Waking up · a few seconds…"`, the box UI was up 90 ms after the click; nav Account / Home 76–78 ms |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280. At 390 the box shows the phone list (FAB), as in D, E |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost` (as in C–E) |

## NEW findings

### 1. [major] The Shell tab never takes the keys: a command typed after clicking it runs the list's shortcuts
- Engine: Chrome and WebKit, desktop 1440. 2 of 2 each, on Compare A and Worktree one.
- Steps: a task with its side panel open on Changes → click the **Shell** tab → type `echo hi`.
- Expected: the terminal has the focus and gets the keys, as v1 did (`web/js/features/panel.js:1014`: opening the Shell
  focuses the terminal; `:370`: so does a shell that (re)starts).
- Actual:
  - After the click, `document.activeElement` is the Shell tab button (`BUTTON.k-tab`).
  - `e` sends `runtime.setState`: toast "Marked done · Undo", and "Remove its worktree?" opens.
  - wisp moves on to the next task (Compare B), and the rest of the typing, `cho hi`, lands in that task's reply box.
  - With Enter, it is sent: my first run queued "ho hello-qa" as a follow-up on Compare B (held, so queued).
  - The same after **Start again** (focus on `BODY`) and on a deep link `?side=shell:…` (focus in the reply box). Only a
    click into the terminal gives it the keys.
- Suspect: `plugins/shell-task/ui/src/side/SidePanel.tsx:196` focuses the selected tab after each change and nothing
  focuses the terminal; the file view got its own fix (E #1).
- Shots: `f2-shell-typed-chromium.png` (Marked done, Remove its worktree?, "cho hi" in Compare B's box),
  `f2-shell-typed-webkit.png`, `f1-shell-chromium.png` (the Shell open, focus on its tab).

### 2. [major] Revert in Changes throws work away at once, with no question and no Undo
- Engine: Chrome and WebKit, desktop 1440, on Worktree one (a worktree with three changed files).
- Steps: side panel → Changes → a hunk's **Revert**; then a file's ⋯ → **Revert file**.
- Expected: v1's way (`web/js/features/changes.js:519-582`).
  - A hunk's Revert goes at once, then toasts "Reverted · Undo", which puts the file's text back.
  - Revert file… asks first, in the danger style: "Revert math.js to origin/main? Every change this task made to math.js
    goes. Undo puts it back." For a new file: "… is new in this task, so reverting deletes it. Undo brings it back."
  - Then the same "Reverted · Undo".
- Actual:
  - Hunk Revert: `git.revert` at once, with no toast at all (none at 150 ms or 1 s). The edit is gone from the
    working tree (`math.js` back to the base).
  - Revert file: no question. On `math.js` its edits went; on the new file `new.txt` the file was deleted, with the toast
    "Reverted new.txt" and no Undo.
  - Nothing brings either back. An untracked new file is lost for good.
- Shots: `f10-hunk-reverted-chromium-light.png`, `f10-hunk-reverted-webkit-dark.png`,
  `f10-file-revert-ask-webkit-dark.png` (after Revert file: the toast, no question).

### 3. [minor] After Commit in Changes, the list drops the files just committed until the page reloads
- Engine: Chrome, desktop 1440. 2 of 2.
- Steps: Worktree one with changes → Changes → type a message → **Commit**.
- Expected: the list keeps git's word, "vs origin/main": the committed files stay listed (they still differ from
  origin/main), as they do after a reload.
- Actual:
  - The commit is made ("Committed 69d0e99"; `git log` has it).
  - First commit (all files): the pane says "No changes yet".
  - Second commit (one new file, `second.txt`): "2 files +4 −1", with `second.txt` gone. It stays so for at least 10 s.
  - The strip's count drops with it ("Changes 2").
  - A reload shows "3 files +5 −1 vs origin/main" and "Changes 3", which matches `git diff --stat origin/main`.
  - It reads as if the commit took the work away.
- Shots: `f11-committed.png` ("No changes yet"), `f13-after-commit-chromium.png`, `f12-after-commit.png` (after reload).

### 4. [minor] Phone: the held run's Trust and start / Skip this run buttons shrank to 28 px (iOS) and 32 px (Android)
- Engine: Chrome iOS 390 L, Android 360 D. Desktop is fine.
- Steps: a held Codex task on a phone (Codex task).
- Expected: buttons a thumb can hit: 44 pt on iOS (HIG) and 48 dp on Android (Material 3), as the phone apps follow.
  E measured these buttons at 44 px (iOS) and 40 px (Android).
- Actual:
  - iOS: Trust and start 137×28, Skip this run 115×28. Android: 149×32 and 134×32.
  - This came with E's fix "the trust card lays out on phones and desktop as v1's" (07d666f2).
  - The card is also inset (x 40–350) from the turn above it (x 24–366).
- Shots: `phone2-chromium-ios-390-light-held.png`, `phone2-chromium-android-360-dark-held.png`.

### 5. [minor] The compare group card runs into the phone's edges, and its "vs Compare" sits off the right column
- Engine: Chrome iOS 390 L, Android 360 D; desktop Chrome 1280 L and WebKit 1440 D. It is the same in E's shots
  (`shots-E/phone-chromium-ios-390-light-home.png`), but it wasn't reported.
- Steps: the Tasks list with a compare group (Compare A / B, the CLI compare).
- Expected:
  - The card sits inside the list's margins like everything else.
  - Its right-hand texts end in the rows' column (the owner's rule: times and pills in fixed columns).
- Actual:
  - On phones the card's box is x 0–390 (iOS) and 0–360 (Android): its border and rounded corners run against the
    screen's edges, while the rows and sections keep their inset.
  - "vs Compare" ends 7–8 px left of the status and time column: iOS 366 vs 373–374, Android 332 vs 343–344, desktop
    367 vs 372–373 (WebKit 1440: 357 vs 362–363).
  - Inside the card, a run's "Finished" ends 2 px right of the other rows' (747 vs 745 at 2×).
- Shots: `f7-group-ios-390-light.png`, `f7-group-android-360-dark.png`, `f7-group-desktop-1280-light.png`.

## Notes (not counted)

- **Open in terminal app** is still refused with "No terminal app on this computer for you here: open the Shell tab
  instead." The cause is my env (`WISP_GRID_URL` set). The ⋯ still offers it (E Notes).
- **Panel toggle label:** the head button keeps "Show side panel (])" while the panel is open (`aria-pressed=true`).
- **Esc in a file view:** the first Esc only leaves the editor (the file stays open, no visible change), the second closes
  the whole panel. Clicking the selected Changes tab while a file is open doesn't go back to the list; the `‹` does.
- **WebKit console:** one run (task2, WebKit 1280 D) logged "The resource …/TaskSide-….js (paint-….js, Tabs-….js) was
  preloaded using link preload but not used within a few seconds". Direct loads of the same pages never logged it.
- **Long-press:** on iOS and Android, CDP touch shows no change until the menu at about 505 ms (as E).
- **Asking rows** read "Starting up" under the title: my gated fake prints nothing before `runtime.ask`, so this is seed.
- **Phone Changes header:** at 360 "vs origin/main" is cut to "v…" next to Collapse all and Open file….
- **Desktop Transcript:** Esc from the inline Transcript still goes Home (B–E note).
- **Phone crew stories strip** still stays fixed under the top bar, with a row cut under the avatars (D, E note; owner's
  call).
- **Grid at 390:** "Asleep · starts the fi…" and "No computers of …" are still cut (C–E note).
- **Forked review's Changes** still lists the parent's edit under `/private/tmp/qaF3/repos/app/` (E note).
- **A merged PR's task** stays in review, with "Changes 1 · PR #1" in the strip; the worktree is removed as the
  question said.
- **Not run:** `qa/parity.mjs` and `qa/layout.mjs`; agent sessions don't run the outer-loop gates.

## Cleanup

My two servers, both started by me, are stopped (`lsof … | xargs kill`, nothing left listening):
- `wisp serve` :4951;
- `wisp-grid dev` :4957, which stopped its boxes.

No other process was touched. Data stays under `/tmp/qaF3` and `/tmp/qa-keep/F`; shots are in `/tmp/qa-keep/shots-F`.
The shared `/tmp/wisp-target-fixer` was used as named and left in place.
