# Audit H: tasks on desktop, phone and Grid web, with the task side panel (wisp v2 @ 86bd4e37, 2026-11-20)

Findings only. No repo file was changed except this report.

**Setup.** This repeats audit G tasks' scope and method (`docs/qa/audit-G-tasks.md`) at HEAD 86bd4e37, in a fresh
`/tmp/qaH-tasks`. Nothing real was touched: no real harness, no service, no paid run, never ports 4477, 4800, 4791,
4329 or 5180, and never `~/.wisp*`.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`.
  The worktree had other sessions' uncommitted edits under `design/v1-reference/` only; no source file differs from HEAD.
  The UI is a `vite build` of this tree in `/tmp/qaH-tasks/web`.
- **Server:** `wisp serve --port 4971`, `WISP_HOME=/tmp/qaH-tasks/wisphome`, `HOME=/tmp/qaH-tasks/home`
  (`/tmp/qaH-tasks/serve.sh`, `env.sh`; the CLI through `/tmp/qaH-tasks/w`).
- **Fakes:** G's QA wrapper for `claude` (`#call`, `SPEC:`/`APPROVE`, RICH, FAILME) with its paths swapped. Every other
  harness is the testkit fake. `gh` is the fake gh. The Codex wrapper got two appended lines, so its fingerprint changed
  and the Codex turns were held, as in G.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`. `WISP_GRID_URL` is the dev grid and
  `WISP_POSTHOG_URL` is `127.0.0.1:9`.
- **Grid:** `wisp-grid dev --port 4977 --data /tmp/qaH-tasks/grid`, dev handles `qg-one`, `qg-two` and `qg-three`.
- **Projects:** `app` (its fake GitHub remote at G's first commit, through `insteadOf`), `plain` (git, no remote) and
  `notes` (no git), all made fresh.
- **Seed:** G's scripts, copied to `/tmp/qa-keep/H-tasks/` with paths and ports swapped (`seed1-3`, `seedE`, `seedF`,
  `seedG`, `seedG2`, `seedheld`, `seedstuff`, `seedwt`), plus H's `seedH` and my `held2/3`, `reseed`.
  - Every state G had: review, done, failed, archived, pinned, for later, unread; running `run1-3` (two with a queued
    follow-up); asking `ask1`, `ask2`; held Codex turns; goal, fork, crew (qabot, docbot), scheduled, rich, 14 fillers.
  - My Stuff (CSV, markdown, snippet, Counter app; the app pinned beside tasks).
  - Compare group `g-qa1`, the CLI compare (`wisp "TOPIC[CLI compare] Make add faster" --agent claude --compare codex`)
    with PR #1 on the fake gh.
  - Worktree one (hand edits to `math.js`, `README.md` and a new `new.txt`). Compare A (`math.js` edited, `extra.txt`
    added). Review me in place with the checkout's `README.md` edited. Forked review's worktree removed.
  - 32 declared goals, factories `strict` and `tight`. Factory runs: default (spec waiting), strict, and a third, "Factory
    stop me" (`tight`, for Stopped).
  - Setup cards `setup1-4` and H's `hsetup1-3`.
  - New for H: a stray git worktree `/tmp/qaH-tasks/stray-wt` made after start (Worktrees › Bring in), and
    `plain/.qa/a.task.yml` with `team.yml` (agent `critic` = codex gpt-5 high) for the file-runner pill.
- **Browsers:** Playwright from `/tmp/pw` with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop: the `__TAURI_INTERNALS__` stub, 1280 and 1440.
  - Phones: the Capacitor stub, iOS 390/360 and Android 360; Chrome uses CDP touch.
  - The phone-width browser: 390 with no stub.
  - Light and dark. Every browser is closed in a `finally` or the exit handler in `lib.mjs`.
- **Feedback** is ms from pointerdown, touchstart or keydown to the first DOM mutation.
  - Chrome, 319 readings in the batch and probe outputs: median 10 ms, p95 32 ms. Over 100 ms: 419 ms (my click on the
    launcher's "in" label, as C–G) and 506–517 ms (scripted long-press; one hand run read 48 ms).
  - WebKit, 125 readings: median 25 ms, p95 82 ms. Over 100 ms: 116 ms (one row open), 119 ms (the harness picker, NEW
    #3) and 428 ms (the "in" label).
  - The machine was loaded during the run (load average 4.4–4.6, another app at about 88 % CPU). WebKit-only timings
    over 100 ms are counted as findings only when the same page's other controls stayed fast (NEW #3).
- **Artifacts:** scripts in `/tmp/qa-keep/H-tasks/` (G's `g-*`, batch and phone scripts; H's `h-*`; this round's `ht-*`),
  their outputs in `/tmp/qa-keep/H-tasks/*.out`, and shots in `/tmp/qa-keep/shots-H/ht-*` (620 files).
- **Noise from my own run:**
  - My first `wisp serve` and `wisp-grid` were stopped by my tool's background time limit, about 10 minutes into the
    first batch. The running, asking and setup-card tasks came back as finished. I deleted and re-seeded those twelve
    (`reseed.mjs`) and re-ran everything after.
  - Setup cards die after about 5 minutes: the wrapper's `fetch` gives up on Node's header timeout while the card waits
    (as G). Allow, Deny and typed deny ran on cards re-seeded just before.
  - As in C–G, many scripted FAILs were my own selectors: virtualized rows, the phone scripts' taps on the collapsed
    small title, and a disabled iOS Archive found before the enabled one. Every scripted FAIL was re-checked by hand
    (`ht-*`, `g-list`, `g-list2`, `g-phonefix`); the tables give the checked result.
  - Rows named "Phone H ios", "Grid task" and "File task", and Filler 3's held Codex turn, came from my scripts.

## Counts

- **Action rows below: 119.** Many rows group several actions, about 240 actions in all.
  - 107 pass.
  - 6 fail: one each for NEW #1–#3, plus three carried over: G #3's Enter on Shell in WebKit, G #4's working-panel
    Stop, and F #1's deep link in WebKit.
  - 6 not tested, each with why.
- The re-check tables (G's 10, E #2, F #1, the side panel audit's 12) come on top of these rows.
- **G tasks' 10 findings, re-checked:** 8 fixed; 2 partly fixed.
  - #3: in WebKit, Enter on Shell leaves the focus on the tab.
  - #4: the working panel's Stop is still 28 / 32 px.
- **Audit E #2:** fixed. The strip scrolls inside the screen with a pinned tab.
- **Audit F #1:** partly fixed. Fixed in Chrome; in WebKit a deep link still puts the focus in the reply box, 5 of 5.
- **Side panel audit's 12:** all 12 hold in Chrome. In WebKit #4 (the arrow walk) breaks at the pinned app's tab (NEW #2).
- **What landed since G:**
  - the file-runner pill, desktop and phones (pass);
  - Stopped status and the factory mark (pass, but see NEW #1);
  - Worktrees › Bring in on open (pass);
  - the phone New task's Goal… and Formula or task file… (pass).
  - Setup cards on channels were not tested: they need a channel service, and this run had none (no network services).
- **NEW findings: 3.** 0 critical, 1 major, 2 minor.
- Console: clean in Chrome and WebKit on every surface. One WebKit run of `stuff.mjs` logged two "Blocked a frame …
  localhost:47301" page errors, but two re-runs in each engine were clean: that was my script reading the app's
  cross-origin frame. Untranslated keys: none.

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub, Web = phone-width browser, no stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## G tasks' findings, E #2 and F #1, re-checked

| # | Finding | Now | How checked |
|---|---|---|---|
| G 1 | Phone-width browser: the first ⋯ sheet titled with another task | **fixed** | C and W Web 390, a fresh browser each. Top row "Pinned one"; tapped "TOPIC[Setup H short] Add a docs helper" → the sheet says "TOPIC[Setup H short] Add a docs helper" (`hs1c`, `hs1w`). Filler 7's sheet says "Filler 7" (shot `ht-sheetback-1-chromium.png`) |
| G 2 | Phone apps' New task: no declared goals, no formula | **fixed** | C iOS 390 L, And 360 D. ⋯ (fb 12–13) = "Goal… · Worktree · Start later · Formula or task file… · Compare… · Start from an issue · Recipes". Goal… (fb 1) lists goal-01…07, "All goals (32)…", "New goal…"; the filter "goal-2" narrows to goal-20…; All goals… opens the picker (fb 2). Formula or task file… opens the file pick, and the composer shows "Formula: a.task.yml ×" (`ht-file-runner-android-360-chromium-dark.png`) |
| G 3 | → along the tabs stops at Shell, then goes into the shell | **partly fixed** | C D 1280/1440: the walk passes Shell, `terminal.input` while walking is resizes only (`size 48x45`), and Enter on Shell → "Terminal input" (`hsh`, `ht-arrow`). W D 1280/1440: arrows no longer type into the shell, but **Enter on Shell leaves the focus on the tab** (`BUTTON:Shell`, 4 of 4: `ht-arrow` and `h-shell` at both widths), so typing goes nowhere until a click. The walk's own break in WebKit is NEW #2 |
| G 4 | Phone answer buttons 28 / 32 px | **partly fixed** | Answer buttons now: C W iOS 390 Allow 55×44, Deny 47×44, Show all 66×44, Approve 73×44, Stop 50×44, One more round 122×44, mathx 59×44, calc 46×44. C And 360: Approve 80×48, One more round 132×48, mathx 65×48. **The working panel's Stop (`task.stop`), on every running or asking task, is still 68×28 on iOS (C and W) and 81×32 on Android** (ask2, run3; `ht-stopsize`). Shot `ht-askstop-ios-390.png` |
| G 5 | Long words run out of their card | **fixed** | C iOS 390: the prompt (`ask-text`, `overflow-wrap: anywhere`) sits in 38–352. Factory strict's question quoting `/tmp/qaH-tasks/…/math.test.js`: ends at 359 of a card ending at 366 (iOS 390), 329 of 336 (And 360) |
| G 6 | Factory reply box foot cut on phones | **fixed** | C iOS 390, And 360: "Stop the run" 69/69 (sw/cw), placeholder "Reply on a station's task to add to it" 231 of 320 / 292. Desktop 1440: "Its stations do the work · Stop the run" and the full placeholder |
| G 7 | Pill says "Factory · Default" | **fixed** | C and W D 1440: the Factory row → "Factory · default"; strict (fb 2 / 13) → "Factory · strict"; reopened, strict is picked |
| G 8 | Factory row shows Claude Code's mark | **fixed** | C D 1440, W D 1280, C iOS 390: all three factory rows have the factory mark and no harness logo |
| G 9 | Setup card Show all: raw JSON, stray "(Show all)", offered when nothing is cut | **fixed** | C D 1440, C iOS 390, And 360: "brief: … uncl…" → Show all (fb 1) → the whole 474 characters, "Show less"; no "(Show all)", no JSON. The two-field and project cards have no Show all |
| G 10 | A row with no summary takes three lines | **fixed** | C D 1440, W D 1280: all 46 rows are 55 px tall; none has an empty second line or a third line. Setup cards with no answer read "Claude Code · Report" on line 2 |
| E 2 | Phone side sheet strip at 360 with a pinned tab | **fixed** | C iOS 360 D: the scroller `panel.tabs` is x 16–344, at rest `data-scroll-end`; a CDP swipe → scrollLeft 33, Thread at 290–342, `data-scroll-start`. And 360 D: scroller 6–344; swipe → 48, Thread at 289–344. The tab list inside keeps its full width (16–377) and is clipped by the scroller, as the fix doc says |
| F 1 | The Shell tab never takes the keys (deep link) | **partly fixed** | C D 1440, 4 of 4: `?side=shell:…` → focus "Terminal input"; `echo z` goes out as `terminal.input`; the reply box stays empty. **W D 1280/1440, 5 of 5 fresh page loads: the focus is on `BODY` at 0.5 and 1.5 s, then the reply box ("Follow up with Claude Code…") from 3 s, and `echo z` lands in the reply box** (`ht-deeplink`). Shot `ht-deeplink-webkit-cmpA-1440.png` |

## Side panel audit's findings, re-checked

| SP # | Finding | Now | How checked |
|---|---|---|---|
| 1 | The strip hid v1's tabs behind `+` | **holds** | C W D 1280/1440: Changes, PR, Shell, Preview, then pinned and Thread; no `+` |
| 2 | Labels hidden under 420 px | **holds** | C W D 1280: "Changes 1 · PR #1 · Shell · Preview · [Counter app] · [Thread]" in 281 px; 1440 the same with more room |
| 3 | A pinned tab's label squeezed the panel's own | **holds** | Closed, Counter app is its icon (28 px); open at 1280, Preview gives way ("[Preview] · Counter app"); a factory run adds "[Team]" |
| 4 | `→` moved one tab, then focus fell to the page | **holds in Chrome; fails in WebKit** | C D: the walk goes all the way round, fb 6–21. W D: see NEW #2 |
| 5 | Background reads toasted raw errors | **holds** | C W D: no toast on the in-place, worktree-gone and no-git tasks |
| 6 | Changes on a task in place diffed the whole checkout | **holds** | C W D: Review me lists only math.js ("edits wisp saw"), not the checkout's `README.md` edit |
| 7 | Changes on a task whose worktree is gone said "No changes yet" | **holds** | C D: Forked review shows "1 file +1 −1 · edits wisp saw · math.js", no toast (`ht-forked`). The strip batch's "No changes yet" was read before the pane loaded |
| 8 | Open file… on a folder git can't list | **holds** | C W D: not offered on the in-place, gone and no-git tasks; offered on the PR task |
| 9 | End shell ended at once | **holds** | C W D: End shell asks; End → Start again → the terminal has the keys. Cancel not re-clicked this round |
| 10 | A reattached shell kept its old size | **holds** | C W D: a shell started at 1440, reopened at 1280, wraps at the panel's width (shot `ht-sp-shell-chromium-1280.png`) |
| 11 | The Shell in a dark box inside a white margin | **holds** | C W D: the terminal fills the pane edge to edge |
| 12 | Head Changes chip on a task in place showed the checkout's diff | **holds** | C W D: no chip on Review me (in place) or Notes one (no git); Compare A's chip "+3 −1" matches the panel's "2 files +3 −1 vs origin/main" |

## Side panel

| Action | Result | Where / how checked |
|---|---|---|
| `]` opens, `]` closes; head button toggles | pass | C W D 1440 on Compare A: fb 5–21. The × was not re-clicked (my selector) |
| Strip at 1280 / 1440, labelled tabs, Thread and a pinned app | pass | SP #1–3 (`gs1`, `gs2`, six tasks each, light and dark) |
| Each tab opens (Changes, PR, Shell, Preview, Counter app, Thread) | pass | C W D; Thread fb 1–11 ("No Notes yet…") |
| `←` / `→` walk the tabs | **fail** | C pass; W fails (NEW #2) |
| Arrow onto Shell starts it without taking the keys | pass | C W D (G #3) |
| Enter on Shell hands it the keys | **fail** | C pass; W leaves the focus on the tab (G #3 partly fixed) |
| Deep link `?side=shell:…` takes the keys | **fail** | C pass; W the reply box (F #1 partly fixed) |
| Changes on a worktree: list, fold, Collapse all, header words | pass | C W D: "1 file +2 −1 vs origin/main" |
| Changes on a task in place / worktree gone / no git | pass | SP #6, #7 |
| Stage a file, and again to unstage | pass | C W D on Compare A: fb 6 / 26, `git.stage`; git `A  extra.txt`, the commit row "1 staged"; again → back |
| Revert a hunk; ⋯ Revert file (a changed and a new file); Undo | pass | C L, W D on Worktree one: hunk Revert fb 3 / 18, "Reverted · Undo", Undo puts it back. Revert file… asks, as v1, on math.js and on new.txt ("is new in this task, so reverting deletes it…"); Undo restores both (`gr1`, `gr2`) |
| Commit from Changes | pass | C D 1440: "Commits all 3" → Commit fb 6, "Committed a06065a"; the pane then reads "3 files +4 −1 vs origin/main", as `git diff --stat origin/main` does |
| PR: card, Merge asks first | pass | C D: "feat: faster add · #1 · Open · Ready to merge · Merge · Checks · Open on GitHub"; Merge asks "Merge this pull request? It squash-merges on GitHub and updates your local base branch…" |
| PR: Merge → confirm | pass | C D: fb 8, `ship.merge`; "#1 · Merged · Merged into main"; local main and the fake origin have `a70b83e feat: faster add`; the worktree was gone within 5 s |
| Shell: start, type, End shell asks, Start again | pass | SP #9 |
| Preview, pinned My Stuff tab | pass | C W D: Counter app tab, `stuff.launch` |
| Pin beside tasks (My Stuff card menu) | pass | C D 1440: menu fb 5; "Pinned Counter app beside tasks · Undo", fb 9 |
| Phone sheet: tabs, Changes rows | pass | C iOS 390 L: "Changes 2 · PR · Shell · Preview · Counter app · Thread"; the rows below |
| Phone sheet strip at 360 with a pinned tab | pass | E #2 |
| Open in terminal app | not tested | refused in this env, as F and G (`WISP_GRID_URL` makes the server count itself headless) |

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows, sections (Pinned, Needs you, Working, Scheduled, For later, Archived) | pass | C W D 1280: "Pinned 1 / Needs you 43 / Working 4 / Scheduled 3 / For later 1 / Archived 1" |
| Fold / unfold a section | pass | C: Needs you 14 → 6 rows, fb 6 |
| Open a row (click) | pass | C by hand: fb 15–27 (4 rows). W: 68–140; see the feedback note |
| Hover Save for later + Undo; hover Mark done + Undo | pass | C fb 9–14; W 35–63 |
| Row ⋯ menu | pass | C W: Filler 7's full menu (15 items) |
| Pin / Unpin | pass | C W: fb 15–59. The script's "pinned=undefined" is its own read, as G |
| Mark read / unread; Save for later + Undo; Archive + Undo | pass | C W: fb 11–51 |
| Rename → Save; Delete → Cancel | pass | C W: fb 9–55 |
| Copy › task ID | pass | C W: "Copied the ID of “Filler 7”" |
| Task options › Set goal, Add to crew, Save as recipe, Thread | pass | C W: fb 2–16 |
| Task options › Side discussion… | pass | C D 1440: fb 4; opens Filler 7 with "/side" in its reply box, focused (v1's `sideDraft`, `row-menu.ts`). Shot `ht-side-discussion-chromium.png` |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C fb 2–34; W 10–93 (`gl3`, `gl4`) |
| ⌘K → type → Enter | pass | C: fb 23 |
| Project switcher → plain → ⌘0 | pass | C: fb 7, 14 rows back |
| Hide / show the list (`[`) | pass | C: fb 1 |
| Asking rows first, saying their question | pass | C W D, phones: "Asks: Claude Code wants to add the project …", "Asks: Approve the spec?…", "Asks: Checks still fail after 1 round: …" |
| Held rows | pass | C W D: "Filler 3 · Waiting for Codex · Held · 1 queued" (my script's Codex reply), Codex task |
| A factory run's row | pass | G #8 |
| A stopped factory run's row says Stopped | pass | C D 1440: "TOPIC[Factory stop me] Tidy names · Factory · tight · Stopped" (audit H orchestration G15); Home lists it under "Stopped" |
| A review row with no summary | pass | G #10 |
| Compare group card | pass | C D 1440: open Compare fb 9 |
| Scheduled section rows | pass | C W: "Plain one · check CI again", the two plans counted in "Scheduled 3" |
| Phone: rows, sections, bottom tabs | pass | C iOS 390 L, And 360 D, W iOS 390 D: tabs fb 6–32 |
| Phone: search → results | pass | C iOS: fb 9; W iOS: 29 |
| Phone: project title ▾ → Projects sheet | pass | C iOS 390 by hand (`gpf1`): fb 14; C And 360: fb 11–12. The scripts' taps hit the collapsed small title, as G |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C iOS 390: ⋯ fb 3, Select fb 0–1, picks fb 5, Archive fb 11, "Archived 2 tasks · Undo", both back. C And 360: the M3 contextual bar "2 selected · ✓ · 🔖 · ⋮", ⋮ fb 2 → "Archive · Delete · Select all", Archive → "Archived 2 tasks · Undo", both back (`ht-select`) |
| Phone: long-press → sheet / menu, drill into Task options | pass | C iOS 390 (`pE-ios`): menu at 48 ms, drill fb 3; And 360: menu at 511 ms, drill fb 3 |
| Phone: swipe right → Mark done + Undo; swipe left → Save for later + Undo | pass | C iOS 390: fb 30 / 47; And 360 (Save for later): fb 32 |
| Phone: pull to refresh | pass | C iOS, And: fb 29–35 |
| Phone-width browser: row ⋯ as a sheet, drill in (Task options), Esc back a level | pass | C W Web 390: fb 3–22; Esc steps back, a second Esc closes |
| Phone-width browser: the first ⋯ sheet's title | pass | G #1 |
| Phone-width browser: a long sheet scrolls to its last item | pass | C W Web 390×844, C 360×640: Filler 7's 15-item sheet ends at Archive; its body scrolls (48 / 221 px) to "Delete task" at 788–832 of 844 and 584–628 of 640 |
| Phone: swipes and pull in WebKit | not tested | no touch injection in WebKit (as G) |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| ⋯ menu (Transcript, Shell, Rerun, Fork, Open in, Rename, Pin, Mark unread, Save for later, Task options, Copy, Share, Archive, Delete) | pass | C W D: fb 2–16 |
| ⋯ Mark unread, Pin / Unpin | pass | C W (`task2-*`): "“Done one” marked unread · Undo"; Pin true → undefined |
| ⋯ Save for later + Undo | pass | C W D by hand: fb 18 / 22, "Saved for later · Undo", forLater set, then cleared by Undo |
| ⋯ Transcript | pass | C W: fb 7–26. Esc then goes Home (note, as B–G) |
| ⋯ Open in › Open in Claude Code | pass | C: fb 2 |
| ⋯ Rerun…, Fork…, Share… open and cancel | pass | C: fb 10–16 |
| Phone-width browser: task ⋯ as a sheet, drills Open in › and Task options › | pass | C Web 390: fb 1–7 |
| Harness picker → Codex → reply → runs on Codex | pass | C W D: placeholder "Follow up with Codex…", reply fb 3 / 9, the turn is Codex (held) |
| Reply: type → Enter → new turn | pass | C D; C iOS/And (turns 1→2, 2→3) |
| Queued follow-up shown, Send now | pass | C W D on run2: "Next · after Claude Code finishes · queued for run2 · Send now" |
| Stop a running turn asks first | pass | C W D on run3: "Stop Claude Code? The run ends where it is. · Cancel · Stop", fb 3 / 21 |
| Waiting task: question card → answer | pass | C: "Which date library should I use?" → option, fb 7, running |
| Held task: "Codex changed since wisp last ran it · Trust and start / Skip this run" | pass | C D; phones: Trust and start 137×44 / 173×40 |
| Phone: back button and system Back | pass | C iOS, And: fb 8–21 |
| Phone: reply box full width | pass | C iOS 24–342 on the task, New task 8–374 |
| Phone: long title in the bar | pass | C iOS 390: `phone.task.bar.title` 81–332, ellipsis |
| Long prompt or question text on phones | pass | G #5 |
| Phone answer buttons (ask, setup card, factory) | pass | G #4 (answer buttons) |
| Phone working panel's Stop | **fail** | G #4 partly fixed: 68×28 iOS, 81×32 Android |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 |

## Factory runs

| Action | Result | Where / how checked |
|---|---|---|
| New task: harness picker → Factory row | pass | C D 1440: fb 16, then 6. "FACTORIES · default (Built in …) · strict · tight". Phone apps: the same in the picker's drill (C iOS 390, And 360: fb 7 / 9) |
| Pick a factory: what the pill says | pass | G #7 |
| Band: stations, the station waiting, round counts | pass | C W D, C iOS 390: "Factory · default · Waiting on you · Spec — Waiting on you: Approve the spec?… · Build · Checks · Review · Ship"; strict "Build · round 3 … Checks · round 3" |
| Band ⋯ (Open Spec, Stop the run) | pass | C W D, C iOS: fb 2–3 |
| Stop the run asks first; Cancel keeps it | pass | C W D, C iOS: "Stop the factory run? Every station still working stops, and the run ends Stopped…", fb 3–5; Cancel fb 1 |
| Stop the run → confirm | pass | C D 1440 on Factory stop me: fb 4, `factory.stop`; the band's Checks reads "stopped"; the row says Stopped. What's left on the page: NEW #1 |
| A stopped run's open question | **fail** | NEW #1 |
| Head ⋯ → Delete asks first | pass | C W D: "Delete this task from wisp? Its stations' tasks go with it…", fb 8–9 |
| Question card: Approve | pass | C D 1440 on strict: fb 6, `runtime.answer`; Build and Checks run three rounds, then "Checks still fail after 3 rounds: … Keep going?" |
| Reply box: placeholder, words to the spec | pass | C D 1440: "Factory · default: its stations do the work. To add to one, reply on its task."; "Also handle floats" fb 9, `runtime.followUp`; the spec station asks again |
| Phone reply box foot | pass | G #6 |
| The run's list row | pass | G #8 |

## Setup cards

| Action | Result | Where / how checked |
|---|---|---|
| The card: who, what, fields, Allow / Deny, "Or type why not below" | pass | C W D, phones: "Claude Code wants to add crew member revbot (codex, plan) · brief … · cwd … · role …" |
| Show all | pass | G #9 |
| Allow | pass | C D 1440: fb 5, `runtime.answer`; revbot is in the crew; the agent hears "The user allowed it. Added revbot to your crew (@revbot)." |
| Deny | pass | W D 1280: fb 6; the agent hears "The user said no: nothing was changed. Don't try it another way…" |
| Typed deny | pass | C D 1440: fb 6; the agent hears the same plus "They said: “not now, I add projects myself”." |
| Asking rows and Home's Needs you say the card | pass | "Asks: Claude Code wants to add the project /tmp/qaH-tasks/repos/plain" |
| A setup card on a channel (ntfy: no buttons; Telegram/Slack/Discord keep them) | not tested | needs a channel service; this run allows no network services |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects → pick | pass | C W (the 419–428 ms is my click on "in", as C–G); the list's switcher → plain sets it too |
| Place, mode, + menu | pass | C W: + fb 10 / 21–35, place fb 8, mode fb 3 / 5–8 |
| Harness picker | **fail** | C pass (fb 10–24); W 102–130 ms (NEW #3) |
| Goal… with 32 goals: 7 shown, filter, All goals (32)…, pick | pass | C D 1440 L, W D 1280 D: Goal… fb 10 / 65, seven goals, "All goals (32)…", "New goal…"; "goal-2" narrows (fb 3 / 16); All goals… fb 3 / 21; goal-17 picked (fb 12 / 43) |
| Phone-width browser: + → Goal… as a drill-in sheet with its filter | pass | C W Web 390: + fb 5 / 9, Goal… fb 8 / 25; "goal-3" → goal-30 |
| Phone apps: Goal… and Formula or task file… | pass | G #2 |
| Formula or task file…: the file-runner pill (new since G) | pass | Picked `plain/.qa/a.task.yml` (`agent: critic`, codex gpt-5 high). C D 1440, W D 1280: the pill reads "critic · Codex · gpt-5 · high", a "Formula: a.task.yml ×" chip, the prompt filled, item fb 3 / 19. C iOS 390, W iOS 390, And 360: "critic · Codex" and the chip. C Web 390: the pill reads "gpt-5 · high" and the chip folds into More options ("Formula: a.task.yml"). No toast, no error |
| Launch with Enter → task page at once | pass | C: fb 3 |
| Needs you on Home | pass | 5 of N, starting with the asking and factory rows |
| Desktop drag strip | pass | C D: `start_drag`, double-click → `zoom_window` |
| Phone New task → Start → task page | pass | C iOS 390 by hand: Start (`phone.new.bar.confirm`) fb 5 → the task page; C And 360 (`phone2`): fb 6 |
| Phone New task composer full width | pass | C iOS 8–374 of 390; And 8–344 of 360 |

## My Stuff, crew, goals, later, worktrees

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff layout; open CSV and markdown; launch an app | pass | C W D: CSV fb 36–43, markdown 15–21, app 16–24 |
| Card menu; Remove + Undo | pass | C W D: fb 5–19 / 10–57 |
| Search filters | pass | "Counter" matches the app and its copies (as C–G) |
| Phone Stuff: tabs, open a doc | pass | C iOS/And (batch) |
| Crew rail → member page; Org; Add to your crew | pass | C D 1440: fb 15 / 13 / 10; revbot (added by a setup card) on the rail |
| Goal task: goal band | pass | C D |
| Settings › Worktrees: a worktree another tool made after start shows with Bring in (new since G) | pass | C D 1440: "1 worktree from git · In app · Not now · Bring in" 369 ms after the pane opened (`ht-worktrees-chromium-light.png`) |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp or a real harness (as C–G) |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Landing and dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: box ⋯ (Copy address, Settings → Agents, API keys) | pass | C 1280, 390: fb 2–4 |
| Add ▾ (A computer of yours, Another cloud machine) | pass | C 1280, 390: fb 1–3 |
| Account page; Getting started → Continue | pass | C 1280, 390: full-page loads (the script's fb is null, as D–G). The nav timing at 390 (`grid4`) hung and was stopped |
| Open the cloud box (asleep) → wisp UI | pass | C 1280, fresh `qg-three`: `data-wake="Waking up · a few seconds…"`, the box UI up at 90 ms |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280 ("Grid task · Finished"); at 390 the box shows the phone list, as D–G |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost` (as C–G) |

## NEW findings

### 1. [major] Stopping a factory run leaves its question open; answering "One more round" is taken and nothing happens
- Engine: Chrome desktop 1440, plus the CLI. 1 of 1 stopped run, and its question was still there after a reload.
- Steps:
  1. A factory run waiting on its loop question. Here "Factory stop me", `tight`: "Checks still fail after 1 round: …
     Keep going? · One more round · Stop".
  2. Band ⋯ → Stop the run → Stop the run.
  3. Reload the page, and run `wisp show <id>`.
  4. Click One more round.
- Expected: the run ends Stopped and its open question goes with it, or answering it says the run is stopped (or
  restarts it). Never a silent no-op (`design/feedback.md`: "Never: silent failure").
- Actual:
  - After Stop (fb 4, `factory.stop`) the band reads "Build · All done · Checks · stopped" and the row says Stopped.
  - The page still shows "Needs you · Checks still fail after 1 round: … Keep going? · One more round · Stop", and so
    does a reload. `runtime.get` still has the ask.
  - `wisp show` prints "▦ tight: ✓ build · ✗ checks · you stopped it", and then "? Asks: … 1. One more round 2. Stop ·
    Answer: wisp reply <id> …".
  - One more round (fb 9, `runtime.answer`) is accepted and the card goes. The band still reads "stopped", no round
    runs, and no toast or line says why.
- Shots: `ht-stopped-task-chromium.png` (the card under the stopped band), `ht-stale-ask-chromium.png` (after a reload),
  `ht-stale-answer-chromium.png` (after One more round).

### 2. [minor] WebKit: the arrow walk along the side panel's tabs drops the focus to the page at the pinned app's tab
- Engine: WebKit desktop 1280 and 1440, 5 of 5 runs (`ht-arrow` ×2, `h-shell` ×2, `g-arrow`; also in `g-strip`'s walk).
  Chrome is fine.
- Steps: a task with the Counter app pinned (the PR task) → its side panel → focus Changes → `→` along the strip.
- Expected: each arrow moves the focus to the next tab and selects it, as in Chrome (SP #4, E #5).
- Actual:
  - "PR #1 → Shell → Preview", then `→` selects Counter app but the focus is on `BODY`.
  - The next `→` still moves on (Thread), and the walk loses the focus again one step later at 1440 (on Changes).
  - Each tab switch in WebKit takes 100–245 ms to show (Chrome 6–21).
  - A keyboard user loses the focus ring and the place in the strip at the pinned tab.
- Shot: `ht-arrow-body-webkit-1440.png`.

### 3. [minor] WebKit: the harness picker takes 102–130 ms to show
- Engine: WebKit desktop 1280, 12 of 12 opens (`ht-hand harness` ×4, `ht-wkfb` ×4, the batch's `home-w` 119 ms).
  Chrome: 10–24 ms.
- Steps: Home → the launcher's harness pill.
- Expected: something visible within 100 ms (ADR-0017; `qa/feedback.mjs` fails over 100). G measured 24 ms in WebKit.
- Actual:
  - 122, 118, 114 and 105 ms in one browser. The same page's + menu answered in 21–28 ms and the mode menu in 5–8 ms, in
    the same runs.
  - The picker now holds the harness list, the Factory section and each harness's models.
  - The machine was loaded (see Setup), but the other controls' numbers show that this one is slower on its own.
- Evidence: timings only, no shot. Script `/tmp/qa-keep/H-tasks/ht-wkfb.mjs` (it prints the readings), and the batch's
  `/tmp/qa-keep/H-tasks/home-w.out`.

## Notes (not counted)

- **Deliberate, as listed in G:**
  - Ship only on worktree tasks;
  - v1's Needs you count (Home and the switcher count more than the list's section);
  - the working title's shimmer;
  - the 1280 edge where Preview becomes an icon with a pinned app open.
- **Phone-width browser, task file picked:** the harness pill shows only "gpt-5 · high" (the agent's name and harness
  fold away); the chip is in More options. Desktop and the phone apps name "critic · Codex".
- **Phone apps' file-runner pill** shows "critic · Codex" without the model and effort that desktop shows.
- **Stopped run's band** says "stopped" in lower case under Checks, where the row and Home say "Stopped".
- **Desktop head ⋯ on a factory run** still lists Shell, Rerun…, Fork… and Open in (G note).
- **Esc** from the inline Transcript still goes Home (B–G note).
- **Long-press:** CDP touch shows the menu at 505–517 ms in most runs, 48 ms in one (as E–G: the press delay itself).
- **Phone crew stories strip** still stays fixed under the top bar (D–G note; owner's call).
- **Forked review's Changes** still lists the parent's edit under `/private/tmp/qaH-tasks/repos/app/` (E–G note).
- **A launched My Stuff app** is served by wisp on `localhost:47301`, outside my 4900–4999 range: wisp picks that port,
  not me.
- **Not run:** `qa/parity.mjs`, `qa/layout.mjs` and `qa/feedback.mjs`. Agent sessions don't run the outer-loop gates.

## Cleanup

- My two servers, both started by me:
  - `wisp serve` :4971 (`/tmp/qaH-tasks/serve.sh`);
  - `wisp-grid dev` :4977 (`/tmp/qaH-tasks/grid.sh`).
  Both were stopped at the end (`lsof -ti tcp:<port> -sTCP:LISTEN | xargs kill`, pids 20528 and 20666). Nothing listens
  in 4900–4999 now, and no `/tmp/qaH-tasks` process is left. The grid stopped its boxes with it.
- One hung script of mine (`grid4.mjs`, pid 99560, with its Chrome 99563) was killed by pid. Every other browser closed
  in its `finally`.
- No other process was touched. Data stays under `/tmp/qaH-tasks` and `/tmp/qa-keep/H-tasks`; shots are in
  `/tmp/qa-keep/shots-H/ht-*`. The shared `/tmp/wisp-target-fixer` was used as named and left in place.
