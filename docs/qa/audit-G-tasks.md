# Audit G: tasks on desktop, phone and Grid web, with the task side panel (wisp v2 @ be468794, 2026-11-20)

Findings only. No repo file was changed except this report.

**Setup.** This copies Audit F's setup (`docs/qa/audit-F-tasks.md`) and the side panel audit's states
(`docs/qa/audit-side-panel.md`), in a fresh `/tmp/qaG3`. Nothing real was touched: no real harness, no service, never
ports 4477, 4800, 4791 or 4329, and never `~/.wisp*`.
- **Build:** `wisp`, `wisp-fake-agent`, `wisp-fake-gh` and `wisp-grid`, built with `CARGO_TARGET_DIR=/tmp/wisp-target-fixer`
  (incremental, 24 s). HEAD be468794, plus other sessions' uncommitted edits under `design/v1-reference/` (no source
  file differs from HEAD). The UI is a `vite build` in `/tmp/qaG3/web`.
- **Server:** `wisp serve --port 4961` with `WISP_HOME=/tmp/qaG3/wisphome` and `HOME=/tmp/qaG3/home`. Scripts:
  `/tmp/qaG3/serve.sh` and `env.sh`. Nothing in 4900–4999 was busy when I started.
- **Fakes:** every harness points at the fake agent. `claude` goes through audit G orchestration's QA wrapper
  (`#call <tool> <json>` over the run's own MCP, the factory's `SPEC:`/`APPROVE` cues), plus F's RICH and FAILME. Appending a
  line to the `codex` wrapper changed its fingerprint, which held the Codex turns. gh points at the fake gh.
- **Privacy:** `DO_NOT_TRACK=1 WISP_FEEDBACK=0 WISP_TELEMETRY=0 WISP_UPDATE_CHECK=0`; `WISP_GRID_URL` is the dev grid and
  `WISP_POSTHOG_URL` is `127.0.0.1:9`.
- **Grid:** `wisp-grid dev --port 4967 --data /tmp/qaG3/grid --wisp /tmp/qaG3/wisp`, dev handles `qg-one`, `qg-two` and
  `qg-three`.
- **Projects:** `app` (fake GitHub remote through `insteadOf`), `plain` (git, no remote), `notes` (no git), made fresh from
  F's first commit.
- **Seeded tasks:** F's seed scripts with paths, ports and ids swapped (`/tmp/qa-keep/G3/seed1-3.mjs`, `seedE`, `seedF`,
  `seedheld`, `seedstuff`, `seedwt`), plus `seedG.mjs` and `seedG2.mjs`. Every state:
  - review, done, failed (FAILME), archived, pinned, for later, unread;
  - running (gated `run1`, `run2`, `run3`; `run1` and `run2` with a queued follow-up);
  - two asking questions (`runtime.ask` on gated `ask1`, `ask2`);
  - held: two Codex turns (Codex task, Compare B), plus the CLI compare's Codex run;
  - goal, forked, crew (qabot adopted, docbot created), scheduled (`later.add`, two `later.plan`), rich blocks, 14 fillers;
  - My Stuff: CSV, markdown, snippet and an app;
  - **compare group** `g-qa1` (Compare A on Claude Code, Compare B on Codex), plus the CLI compare
    `wisp "TOPIC[CLI compare] Make add faster" --agent claude --compare codex`, each run in its own worktree;
  - **Worktree one**: a plain task in its own worktree, with `math.js`, `README.md` and a new `new.txt` edited by hand;
  - **32 declared goals** (`wisp apply -f`: `goals.yml` with `green-build` and `never`, and audit G orchestration's
    `many.yml`, `goal-01`…`goal-30`);
  - **factories** `strict` and `tight` (`fac.yml`). Two runs: `--factory` (the built-in default) waiting on "Approve the
    spec?", and `--factory strict` asking "Checks still fail after 3 rounds … Keep going?" (the check needs a test
    file the project doesn't have);
  - **setup cards** (ADR-0113): runs whose agent calls `wisp_crew_add` (one with a long `brief`, one short) or
    `runtime_projectAdd` over its MCP and waits on the card.
- **Side panel states:** a worktree with real edits (Compare A: `math.js` changed, `extra.txt` added); a task in place
  while the checkout is dirty (Review me; `README.md` edited); a task whose worktree was removed (Forked review); a folder
  without git (Notes one); an open PR (`ship.ship` on the CLI compare's Claude run, PR #1 on the fake gh); the Counter app
  pinned beside tasks; the Thread tab.
- **Browsers:** Playwright from `/tmp/pw`, with system Chrome (`channel:'chrome'`) and WebKit.
  - Desktop is stubbed with `__TAURI_INTERNALS__`, 1280 and 1440.
  - Phones use the Capacitor stub (`ios` / `android`) at 390 and 360. Chrome uses CDP touch.
  - The phone-width browser is the same pages at 390 with no stub (the web's sheet menus).
  - Light and dark.
  - Every browser is closed in a `finally` (or the exit handler in `lib.mjs`).
- **Feedback** is the ms from pointerdown, touchstart or keydown to the first DOM mutation. Over the 193 readings in the
  batch outputs: median 5 ms, p95 31 ms. The hand probes read 0–26 ms. Over 100 ms, the same as F:
  - 407 and 411 ms: my script clicking the launcher's "in" label;
  - 504–506 ms: long-press on iOS and Android (Notes).
- **Artifacts:**
  - Scripts: `/tmp/qa-keep/G3/*.mjs`. These are F's scripts with paths, ports and ids swapped, plus new probes `g-*.mjs`.
  - Raw step outputs: `/tmp/qa-keep/G3/*.out`.
  - As in C–F, many scripted FAILs were my own selectors: the list is virtualized, the phone scripts tapped the collapsed
    small title, and WebKit's list script stalled after one stuck menu. Every FAIL was re-checked by hand (`g-list.mjs`,
    `g-list2.mjs`, `g-phonefix.mjs`, `g-task.mjs`), and the tables give the checked result.
  - Seed noise from my own scripts:
    - My first setup cards died with "node:internal/process/promises:332". My wrapper's `fetch` gave up after Node's
      300 s header timeout while the card waited, so the run crashed. Those rows read "Failed".
    - Editing the wrapper changed Claude's fingerprint, so the next three runs were held until `wisp held … start`.
    - The phone scripts' launches ("Phone G ios 390", "Launched B") are mine.
- **Shots:** `/tmp/qa-keep/shots-G/` (588 files; this round's new ones are named `g-*`).

## Counts

- **Action rows below: 111.** Many rows group several related actions, about 230 actions in all.
  - 94 pass.
  - 12 fail: one for each NEW finding (#1–#10; #8 shows in two tables), and audit E #2, which still fails.
  - 5 not tested, each with why.
- The re-check tables (Audit F's 5, Audit E #2, the side panel audit's 12) come on top of these rows.
- **Audit F's 5 findings, re-checked:** 4 fixed; 1 partly fixed (#1: a click or Start again focuses the terminal, but
  a deep link still puts the focus in the reply box).
- **Audit E #2 (phone sheet strip at 360 with a pinned tab):** still fails.
- **Side panel audit's 12 findings, re-checked:** all 12 hold. SP #4, the arrow-key walk, holds up to Shell; past it, see
  NEW #3.
- **NEW findings: 10.** 0 critical, 3 major, 7 minor (#1–#10 below).
- Console: clean in Chrome and WebKit on every surface, except WebKit's "preloaded but not used" warnings on one phone run
  (as in F). Untranslated keys: none.

Legend:
- Surfaces: C = Chrome, W = WebKit.
- Platform stubs: D = desktop (Tauri stub), iOS / And = phone stub, Web = phone-width browser, no stub.
- Widths are in px; L/D = light/dark.
- "fb" is feedback in ms.

## Audit F's findings, and E #2, re-checked

| # | Finding | Now | How checked |
|---|---|---|---|
| F 1 | The Shell tab never takes the keys | **partly fixed** | C W D 1440: after a click on Shell the focus is the terminal (`TEXTAREA` "Terminal input"); `echo hi` goes out as `terminal.input`, nothing is marked done, state stays review. After End shell → Start again the same. **A deep link `?side=shell:…` still puts the focus in the reply box** ("Follow up with Claude Code…"), so what you type lands there (harmless now: no shortcut runs). Shots `f2-shell-typed-chromium.png`, `g-shell2-chromium.png` |
| F 2 | Revert throws work away with no question and no Undo | **fixed** | C L, W D 1440 on Worktree one. Hunk Revert: fb 3, `git.revert`, toast "Reverted · Undo"; Undo puts the file back, byte for byte. ⋯ Revert file… asks first, as v1: "Revert math.js to origin/main? Every change this task made to math.js goes. Undo puts it back." On the new file: "new.txt is new in this task, so reverting deletes it. Undo brings it back." Revert file → "Reverted · Undo", and Undo restores it, the untracked file included. Shots `g-revert-ask-new.txt-webkit-dark.png` |
| F 3 | After Commit, the list drops the committed files | **fixed** | C D 1440: Commit → "Committed 1a557e6"; the pane reads "3 files +4 −1 vs origin/main", which is what `git diff --stat origin/main` says |
| F 4 | Phone: held run's buttons shrank to 28 / 32 px | **fixed** | C W iOS 390: Trust and start 137×44, Skip this run 115×44. C And 360: 173×40 and 158×40, stacked (E's sizes). The card is still inset (x 40–350 against the turn's 24–366). The answer buttons on other phone cards are another matter: NEW #4. Shot `g-phonecard-codex-chromium-ios-390-light.png` |
| F 5 | The compare group card runs into the phone's edges; "vs Compare" off the right column | **fixed** | C iOS 390, And 360, desktop C 1280, W 1440: the card sits inside the list's margins; "Compare" ends with the time column (iOS 374 vs 373–374, And 344 vs 343–344, desktop 373 vs 372, WebKit 363 vs 362). Shot `f7-group-ios-390-light.png` |
| E 2 | Phone side sheet: with a pinned tab, the strip runs past the screen at 360 | **still fails** | C iOS 360 D: tab list x 16–377 on a 360 px screen; C And 360 D: x 6–392, Thread off the screen. `scrollWidth == clientWidth`. Shots `e29-phone-sheet-ios-360-dark.png`, `e29-phone-sheet-android-360-dark.png` |

## Side panel audit's findings, re-checked

| SP # | Finding | Now | How checked |
|---|---|---|---|
| 1 | The strip hid v1's tabs behind `+` | **holds** | C W D 1280/1440: Changes, PR, Shell, Preview, then pinned and Thread; no `+` |
| 2 | Labels hidden under 420 px | **holds** | C W D 1280: "Changes 1 · PR #1 · Shell · Preview · [Counter] · [Thread]" (281 px row, nothing scrolls); 1440 the same with more room |
| 3 | A pinned tab's label squeezed the panel's own | **holds** | Closed, Counter app is its icon; open at 1280, Preview gives way ("Changes 2 PR Shell [Preview] Counter app [Thread]"), as F |
| 4 | `→` moved one tab, then focus fell to the page | **holds up to Shell** | C W D: Thread → Changes → PR #1 walks; onto Shell the focus goes into the terminal (NEW #3) |
| 5 | Background reads toasted raw errors | **holds** | C W D: no toast on the worktree-gone, in-place and no-git tasks |
| 6 | Changes on a task in place diffed the whole checkout | **holds** | C W D: Review me lists only math.js ("edits wisp saw"), not the user's `README.md` edit |
| 7 | Changes on a task whose worktree is gone said "No changes yet" | **holds** | C W D: Forked review shows the edits wisp saw, no toast |
| 8 | Open file… on a folder git can't list | **holds** | C W D: not offered on the in-place, gone and no-git tasks |
| 9 | End shell ended at once | **holds** | C W D: "End the shell? Whatever runs in it stops with it. · Cancel · End shell" (fb 0–9); Cancel keeps it; End → "Shell ended · Start again" |
| 10 | A reattached shell kept its old size | **holds** | C W D 1280: a shell started at 1440 refits to the panel; nothing past the edge |
| 11 | The Shell in a dark box inside a white margin | **holds** | C W D: the terminal fills the pane |
| 12 | Head Changes chip on a task in place showed the checkout's diff | **holds** | C W D: no chip on in-place, gone and no-git tasks; Compare A "+3 −1" matches the panel |

## Side panel

| Action | Result | Where / how checked |
|---|---|---|
| `]` opens, `]` and × close; head button toggles | pass | C W D (batch `task-*`), fb 1–10 |
| Strip at 1280 / 1440, labelled tabs, Thread and a pinned app | pass | C W D L/D, six tasks each: PR task 1440 "Changes 1 · PR #1 · Shell · Preview · [Counter] · [Thread]" (349 px), 1280 the same in 281 px; factory run adds "[Team]"; never wider than its row |
| Strip with a pinned app, closed / open | pass | SP #3 |
| Each tab opens (Changes, PR, Shell, Preview, Counter app, Thread) | pass | C W D; Thread fb 1–3 ("No Notes yet…") |
| `←` / `→` walk the tabs | **fail** | C W D 1280/1440 (#3) |
| Shell tab takes the keys | pass | F #1 (click, Start again; deep link: see the re-check) |
| Changes on a worktree: list, fold, Collapse all, header words | pass | C W D: "1 file +2 −1 vs origin/main" |
| Changes on a task in place / worktree gone / no git | pass | SP #6, #7 |
| Viewed | pass | C D: fb 1, "1 of 3 viewed" |
| Stage a file, and again to unstage | pass | C W D on Compare A: fb 4–6, `git.stage`; git `A  extra.txt`, the commit row "1 staged"; again → back. Worktree one, with nothing uncommitted after the commit, offers no Stage (right) |
| Revert a hunk; ⋯ Revert file (a changed and a new file); Undo | pass | F #2 |
| Commit from Changes | pass | F #3 |
| `⋯`: Copy path (the ⋯ turns into "Copied") | pass | C D: fb 1 |
| PR: card, Merge asks first (cancelled) | pass | C W D 1280/1440: "feat: faster add · #1 · Open · Ready to merge · Merge · Checks · Open on GitHub"; Merge asks "Merge this pull request? It squash-merges on GitHub and updates your local base branch…" |
| PR: Merge → confirm | pass | C D 1440: fb 3, `ship.merge`; "#1 · Merged · Merged into main"; local main and the fake origin have the squash commit; the worktree is removed |
| PR tab label | pass | "PR #1" (61 px at 1440, 44 px at 1280) |
| Shell: start, type, End shell asks, Start again | pass | SP #9; F #1 |
| Preview, pinned My Stuff tab | pass | C W D: Counter app fb 1 (`stuff.launch`) |
| Pin beside tasks | pass | C D: "Pinned Counter app beside tasks · Undo", fb 5 |
| Phone sheet: tabs, Changes rows | pass | C iOS 390 L, And 390 L: Changes 2 · PR · Shell · Preview · Counter app · Thread; extra.txt and math.js whole, Stage · Viewed · ✎ · ⋯ below |
| Phone sheet strip at 360 with a pinned tab | **fail** | E #2 |
| Open in terminal app | not tested | refused in this env, as F (`WISP_GRID_URL` makes the server count itself headless) |

## Task list

| Action | Result | Where / how checked |
|---|---|---|
| Rows, sections Pinned and Needs you, more below | pass | C D 1280 L, W D 1440 D |
| Fold / unfold a section | pass | C W: Needs you 13 → 6 rows, fb 4–7 |
| Open a row (click) | pass | C W, by hand: fb 17–26 |
| Hover Save for later + Undo; hover Mark done + Undo | pass | C W, by hand: fb 5–16 |
| Row ⋯ menu | pass | C W: fb 2 |
| Pin / Unpin | pass | C W, by hand: fb 6–12 (the script's "pinned=undefined" was its own read; the menu then offers Unpin) |
| Mark read / unread; Save for later + Undo; Archive + Undo | pass | C W, by hand: fb 4–10 |
| Rename → Save; Delete → Cancel | pass | C W: fb 3–12 |
| Copy › task ID, resume command | pass | C W: toast, clipboard |
| Task options › Set goal, Side discussion, Add to crew, Save as recipe, Thread | pass | C W: fb 1–3 |
| Open, Changes, Shell, Rerun…, Fork…, Share…, Open project in editor | pass | C: fb 1–17 |
| Delete → Delete (Filler 13) | pass | C (batch) |
| ⌘K → type → Enter | pass | C W: fb 7, Filler 11 opens |
| Project switcher → plain → ⌘0 | pass | C W: fb 5 |
| Hide / show the list (`[`) | pass | C W: fb 0 |
| Asking rows first, saying their question | pass | C W D, phones: "Asks: Claude Code wants to add crew member revbot…", "Asks: Approve the spec?…", "Asks: What should the new module be called?" (Notes: the factory loop row) |
| Held rows | pass | C W D: "Codex task · Waiting for Codex · Held", the CLI compare's Codex run "Held" |
| A factory run's row | **fail** | C W D, phones (#8) |
| A review row with no summary | **fail** | C W D 1280/1440 (#10) |
| Compare group card | pass | F #5 |
| Phone: rows, sections, bottom tabs | pass | C iOS 390 L, And 360 D, W iOS 390 D: tabs fb 3–6 |
| Phone: search → results | pass | C iOS: fb 4 |
| Phone: project title ▾ → Projects sheet | pass | C iOS 390 by hand (the large title): fb 5; the scripts tapped the collapsed small one |
| Phone: bar ⋯ → Select → pick 2 → Archive → Undo | pass | C iOS 390: fb 11, "Archived 2 tasks · Undo", both back |
| Phone: long-press → sheet / menu, drill into Task options | pass | C iOS 390, And 360: menu at 505 ms, drill fb 2 |
| Phone: swipe right → Mark done + Undo; swipe left → Save for later + Undo | pass | C iOS 390: fb 33 / 32 |
| Phone: pull to refresh | pass | C iOS: fb 35 |
| Phone-width browser: row ⋯ as a sheet, drill in (Task options), Esc back a level | pass | C W Web 390: fb 1–2; the drill has a `‹ Task options` head; Esc steps back, a second Esc closes |
| Phone-width browser: the first ⋯ sheet's title | **fail** | C W Web 390 (#1) |
| Phone-width browser: a long sheet scrolls to its last item | pass | C Web 390×844, 360×640: the task ⋯ › Task options sheet body scrolls; "Keep worktree" ends at 832 of 844 |
| Phone: swipes and pull in WebKit | not tested | no touch injection in WebKit |

## Task page

| Action | Result | Where / how checked |
|---|---|---|
| ⋯ menu (Transcript, Shell, Rerun, Fork, Open in, Rename, Pin, Mark unread, Save for later, Task options, Copy, Share, Archive, Delete) | pass | C W D: fb 1–5 |
| ⋯ Mark unread + Undo, Pin / Unpin, Save for later + Undo | pass | C W (batch `task2-*`; the scripted FAILs read their state before the toast) |
| ⋯ Transcript | pass | C W: fb 3–6. Esc then goes Home (note, as B–F) |
| ⋯ Open in › Open in Claude Code | pass | C W: drill fb 2 |
| Phone-width browser: task ⋯ as a sheet, drills Open in › and Task options › | pass | C Web 390: fb 1–2 |
| Harness picker → Codex → reply → runs on Codex | pass | C D 1280: placeholder "Follow up with Codex…", reply fb 1, the turn is Codex (held, by the changed wrapper) |
| Reply: type → Enter → new turn | pass | C D; C iOS/And |
| Queued follow-up shown, Send now | pass | C D on run2: "Next · after Claude Code finishes · queued for run2 · Send now" |
| Stop a running turn asks first | pass | C D on run3: "Stop Claude Code? The run ends where it is. · Cancel · Stop", fb 2 |
| Waiting task: question card → answer | pass | as F (ask1, ask2) |
| Held task: "Codex changed since wisp last ran it · Trust and start / Skip this run" | pass | C D 1280; phones F #4 |
| Phone: back button and system Back | pass | C iOS: fb 5–6 |
| Phone: reply box full width | pass | C iOS 8–382 |
| Long prompt or question text on phones | **fail** | C iOS 390, And 360 (#5) |
| Phone answer buttons (ask, setup card, factory) | **fail** | C W iOS 390, C And 360 (#4) |
| Thumbs up / down | not tested | hidden by WISP_FEEDBACK=0 |

## Factory runs (new since F)

| Action | Result | Where / how checked |
|---|---|---|
| New task: harness picker → Factory row | pass | C D 1440: fb 4 (11 to open). "FACTORIES · default (Built in · spec → build → checks → review → ship) · strict · tight", "How factories work" link. Phone apps: the same in the picker's drill (C iOS 390, And 360: fb 5) |
| Pick a factory: what the pill says | **fail** | C D 1440 (#7) |
| Band: stations, the station waiting, round counts | pass | C W D 1280/1440, C iOS 390, And 360: "Factory · default · Waiting on you · Spec — Waiting on you: Approve the spec?… · Build · Checks · Review · Ship"; strict "Build · round 3 … Checks · round 3" |
| Band ⋯ (Open Spec, Open Build, Stop the run) | pass | C W D, phones: fb 2–7 |
| Stop the run asks first; Cancel keeps it | pass | C W D, iOS, And: "Stop the factory run? Every station still working stops, and the run ends Stopped. Its stations' tasks and worktrees are kept. · Cancel · Stop the run", fb 3–5; Cancel fb 1–2 (audit G orchestration G13 fixed) |
| Head ⋯ → Delete asks first | pass | C W D: "Delete this task from wisp? Its stations' tasks go with it, and their worktrees that have nothing uncommitted…", fb 7–9 |
| Question card: Approve / Stop; One more round / Stop | pass | C W D (G orchestration: Approve 5 ms) |
| Reply box: placeholder, words to the spec | pass | C D 1440: "Factory · default: its stations do the work. To add to one, reply on its task."; "Also handle floats" → fb 9, `runtime.followUp`; the spec station runs "Change the spec: Also handle floats …" and asks again |
| Phone reply box foot | **fail** | C iOS 390, And 360 (#6) |
| The run's list row | **fail** | #8 |

## Setup cards (new since F)

| Action | Result | Where / how checked |
|---|---|---|
| The card: who, what, fields, Allow / Deny, "Or type why not below" | pass | C W D, phones: "Wants to change your setup · Claude Code wants to add crew member revbot (codex, plan) · brief … · cwd … · role …" |
| Show all | **fail** | C W D, phones (#9) |
| Allow | pass | C D 1440: fb 4, `runtime.answer`; revbot joins the crew; the agent hears "The user allowed it. Added revbot to your crew (@revbot)."; the card goes |
| Deny | pass | W D 1280: fb 6; the agent hears "The user said no: nothing was changed. Don't try it another way…" |
| Typed deny | pass | C D 1440: "not now, I add projects myself" + Enter, fb 5; the agent hears the same plus "They said: “not now, I add projects myself”." |
| Asking rows and Home's Needs you say the card | pass | "Asks: Claude Code wants to add the project /tmp/qaG3/repos/plain" |

## Home and New task

| Action | Result | Where / how checked |
|---|---|---|
| Launcher project chip → Projects → pick | pass | C W (the 407/411 ms was my click on "in", as C–F) |
| Place, mode, harness, + menu | pass | C W: + fb 5–8, "Attach files · Start from an issue… · Recipes · Goal… · Delegate… · Compare with another agent… · Formula or task file… · Start later" |
| Goal… with 32 goals: 7 shown, filter, All goals (32)…, pick | pass | C D 1440 L, W D 1280 D: Goal… fb 10–11, seven goals then "All goals (32)…" and "New goal…"; the filter box narrows ("goal-2" → goal-20…29, fb 2); All goals… opens a searchable picker (fb 2–4); picking goal-17 → chip "Goal: goal-17 ×" and "Done when …" (fb 8–10). Shots `g-goal-menu-*`, `g-goal-all-*`, `g-goal-picked-*` |
| Phone-width browser: + → Goal… as a drill-in sheet with its filter | pass | C Web 390: + fb 1, Goal… fb 3, "‹ Goal · Filter by name or what done means · goal-01 … · All goals (32)… · New goal…"; the filter narrows |
| Phone apps: Goal…'s declared goals; Formula or task file… | **fail** | C iOS 390, And 360 (#2) |
| Formula or task file… opens a file pick | pass | C D 1440: fb 2, the desktop file dialog through the Tauri stub (no web dialog) |
| Launch with Enter → task page at once | pass | C W: fb 2 |
| Needs you on Home | pass | 5 of N, starting with the setup cards and factory questions |
| Desktop drag strip | pass | C W D (batch `home-*`) |
| Phone New task → Start → task page | pass | C iOS 390: fb 5 |
| Phone New task composer full width | pass | C iOS 8–374 of 390 |

## My Stuff, crew, goals, later

| Action | Result | Where / how checked |
|---|---|---|
| My Stuff layout; open CSV and markdown; launch an app | pass | C W D (batch `stuff-*`) |
| Card menu; Pin beside tasks; Remove + Undo | pass | C D: fb 3–5 |
| Search filters | pass | "Counter" matches the app and its copies (as C–F) |
| Phone Stuff: tabs, open a doc | pass | C iOS/And (batch) |
| Crew rail → member page; Org; Add to your crew | pass | C W D; revbot (added by a setup card) on the rail |
| Goal task: goal band | pass | C W D, phones |
| Scheduled section rows | pass | as F (the batch's FAIL was the virtualized row) |
| Publish / Make a pane | not tested | needs a grid sign-in in the local wisp or a real harness (as C–F) |

## Grid web (`wisp-grid dev`)

| Action | Result | Where / how checked |
|---|---|---|
| Landing and dev sign-in by handle | pass | C 1280 L, 390 D |
| Home: box ⋯ (Copy address, Sleep now, Settings → Agents, API keys) | pass | C 1280, 390: fb 2 |
| Add ▾ (A computer of yours, Another cloud machine) | pass | C 1280, 390: fb 1 |
| Account page; Getting started → Continue | pass | C 1280, 390 (full-page loads; the script's fb is null, as D–F); nav Account / Home 60–74 ms to DOMContentLoaded |
| Open the cloud box (asleep) → wisp UI | pass | C 1280, fresh `qg-three`: `data-wake="Waking up · a few seconds…"`, the box UI was up at 90 ms |
| Task in the box: launch → runs on the fake → Finished | pass | C 1280; at 390 the box shows the phone list, as D–F |
| Grid in WebKit | not tested | WebKit can't resolve `*.wisp.localhost` (as C–F) |

## NEW findings

### 1. [major] Phone-width browser: the first ⋯ sheet after a page load is titled with another task
- Engine: Chrome and WebKit, the browser at 390 with no stub. 4 of 4 page loads.
- Steps: load the Tasks page → tap the ⋯ of any row but the top one (gate ask1, Pinned one, Setup deny).
- Expected: the sheet's title is the tapped task's.
- Actual:
  - The title is the list's top row, "Filler 12", while the items are the tapped row's: on gate ask1 the sheet says
    "Filler 12" and offers "Pin", though Filler 12 is pinned.
  - The actions hit the tapped task: Copy task ID copied "TOPIC[Setup card] …"'s ID under a sheet titled "Filler 12".
  - Delete task's confirm says only "Delete this task from wisp?", so a user deleting a task sees another task's name.
  - Every later ⋯ on the same page load has the right title.
  - Desktop menus have no title, so they don't show it.
- Shots: `g-sheetfirst-chromium.png` (gate ask1 → "Filler 12"), `g-sheet-drill-Taskoptions-chromium-web-390-light.png`
  (the Setup card row highlighted, "Filler 12" on the sheet).

### 2. [major] Phone apps: New task can't use the declared goals or a formula
- Engine: Chrome, iOS 390 L and Android 360 D (Capacitor stub).
- Steps: New task → ⋯ → Goal.
- Expected: the same as desktop and the phone-width browser: Goal… lists the most recent goals, a filter, "All goals
  (32)…" and "New goal…" (owner-list #63). Formula or task file… is there too (owner-list #61). The phone apps keep every
  feature.
- Actual:
  - Goal adds a free-text "Done when" line (placeholder "all tests pass and it works on a phone") and nothing else.
    Tapping or typing in it offers no declared goal, so the 32 goals can't be picked.
  - The ⋯ menu is "Goal · Worktree · Start later · Compare with another agent… · Start from an issue · Recipes", with no
    Formula or task file….
- Shots: `g-phpick-goal-chromium-android-360-dark.png`, `g-phpick-more-chromium-ios-390-light.png`.

### 3. [major] `→` along the side panel's tabs stops at Shell, and the next arrow keys go to the shell
- Engine: Chrome and WebKit, desktop 1280 and 1440. 2 of 2 each.
- Steps: a task's side panel → focus the selected tab → `→` along the strip.
- Expected: the arrow keys walk the tabs (SP #4, E #5); a click or Enter on Shell may hand it the keys.
- Actual:
  - Thread → Changes → PR #1 walk, then `→` selects Shell. That starts the shell and moves the focus into the terminal
    (F #1's fix).
  - Each later `→` goes out as `terminal.input`, a cursor key in the shell. The walk can't pass Shell, and getting out
    takes a mouse.
- Shot: `g-arrow-chromium.png`.

### 4. [minor] Phone: the answer buttons on question cards are 28 px (iOS) and 32 px (Android)
- Engine: Chrome and WebKit iOS 390; Chrome Android 360.
- Steps: a task asking a question (ask2), a setup card (Setup phone), a factory run (Factory default).
- Expected: thumb-sized buttons, 44 pt on iOS and 48 dp on Android, as the held card got back (F #4).
- Actual:
  - iOS: Allow 55×28, Deny 47×28, Show all 66×28, Approve 73×28, Stop 50×28, mathx 59×28, calc 46×28.
  - Android: 32 px tall (Allow 60×32, Approve 80×32, mathx 65×32).
  - The card's own "Stop" is 28 / 32 px too.
- Shots: `g-phonecard-setup4-chromium-ios-390-light.png`, `g-phonecard-fac1-chromium-android-360-dark.png`.

### 5. [minor] Phone: long words in a prompt or a question run out of their card and are cut
- Engine: Chrome iOS 390 L, Android 360 D.
- Steps: a task whose prompt holds a long path or JSON (Setup card), or a factory question quoting a path (Factory
  strict's "Cannot find module '/private/tmp/qaG3/wisphome/worktrees/app-…/math.test.js'").
- Expected: long words break inside the card, so everything can be read.
- Actual:
  - The prompt bubble's text runs to x 446 on a 390 screen, clipped by `card.body` (`"cwd":"/tmp/qaG…`).
  - The factory question's text runs to x 367 on a 360 screen (`…/math.test.js` cut).
  - What's cut can't be read or scrolled to.
- Shots: `g-setup1-chromium-android-360-dark.png`, `g-fac-stopask-chromium-android-360-dark-fac2.png` (the question under
  the dialog).

### 6. [minor] Phone: a factory run's reply box cuts "Stop the run" and its hint
- Engine: Chrome iOS 390 L, Android 360 D.
- Steps: open a factory run on a phone.
- Expected: the foot's controls whole or folded into More (ADR-0012: control text never cut or wrapped).
- Actual:
  - iOS: the hint shows "Its …" and "Stop the run".
  - Android: the hint (`composer.hint`, 71 px wide, 75 px of content) drops "Its stations do the work" and shows "Stop the
    ru", cut with no ellipsis (`text-overflow: clip`).
  - The placeholder is cut at "To add t…" on both.
- Shots: `g-fac1-chromium-ios-390-light.png`, `g-fac-stopask-chromium-android-360-dark-fac2.png`.

### 7. [minor] Picking a factory doesn't show which one: the pill says "Factory · Default" for every factory
- Engine: Chrome desktop 1440.
- Steps: New task → harness picker → Factory → strict.
- Expected: the pill names the factory that will run ("Factory · strict"), as the task page's details do.
- Actual:
  - Choosing the Factory row picks "default" and the pill reads "Factory Default".
  - After picking strict (fb 2), the pill still reads "Factory Default". Reopened, the picker shows strict as picked.
  - "Default" reads as the model's word ("Whatever Claude Code is set to").
- Shots: `g-factory-row-chromium-desktop-1440-light.png`, `g-facpick-chromium.png`.

### 8. [minor] A factory run's list row carries Claude Code's mark
- Engine: Chrome and WebKit desktop, phones.
- Steps: the Tasks list with a factory run (Factory default, Factory strict).
- Expected: ADR-0114 rule 14, "Factory · <name>, never a harness"; the run's page says "Factory · default".
- Actual: the row's line shows Claude Code's icon before "Asks: Approve the spec?…". The run's `agent` is `claude`
  (`runtime.get`).
- Shot: `row-Factory_default-0-desktop-1440.png`.

### 9. [minor] A setup card's Show all shows raw JSON, and offers itself when nothing is hidden
- Engine: Chrome and WebKit desktop, phones.
- Steps: a setup card with a long field (revbot's brief), and one with two short fields (helper).
- Expected: Show all only when something is cut, and the whole values in the card's words.
- Actual:
  - The cut field ends "You … (Show all)", plain text that does nothing, above a separate "Show all" button.
  - Show all opens a JSON dump (`{ "agent": "codex", "brief": "…", "cwd": …, "mode": "plan" }`). The summary keeps its
    "(Show all)" while the button reads "Show less".
  - On the helper card (cwd and role, nothing cut), and on a project card (one path), Show all is still offered.
- Shots: `g-setup-showall-chromium-desktop-1440-light-setup1.png`, `g-phonecard-setup4-chromium-ios-390-light.png`.

### 10. [minor] Desktop: a finished row with no summary takes three lines, its status pill alone on the second
- Engine: Chrome and WebKit desktop 1280/1440. Phones put the harness on line 2 and are fine.
- Steps: a task whose last answer had no wisp block, so the row has no summary (my first "Setup card" run: status
  "Report").
- Expected: two lines like the rows around it, the pill beside the harness.
- Actual: title; then an empty line with only "• Report" at the right; then "Claude Code". The row is a line taller than
  its neighbours.
- Shot: `row-Setup_card-1-desktop-1440.png`.

## Notes (not counted)

- **Deliberate, as listed:**
  - Ship only on worktree tasks;
  - v1's Needs you count (Home "5 of 43" and the switcher's "43 need you", against the list's "Needs you 39");
  - the working title's shimmer;
  - the 1280 edge where Preview becomes an icon with a pinned app open.
- **Factory loop question.** Factory strict's row reads "Asks: Checks still fail after 3 rounds: The checks failed:":
  the row ends on a colon. The question itself quotes a node stack trace with the worktree's full path and cuts it with
  "a…. Keep going?".
- **Phone-width browser, row ⋯ on asking rows.** It has no Mark read, Save for later or Archive. Review rows have them.
- **Toast over a sheet.** On the phone-width browser, a toast ("Copied the ID of …") covers the Goal sheet's last row
  ("New goal…") while it shows.
- **Desktop head ⋯ on a factory run** still lists Shell, Rerun…, Fork… and Open in.
- **Open in terminal app** is still refused in this env (as E, F).
- **Esc** from the inline Transcript still goes Home (B–F note).
- **Long-press:** on iOS and Android, CDP touch shows no change until the menu at about 505 ms (as E, F).
- **Phone crew stories strip** still stays fixed under the top bar, with a row cut under the avatars (D–F note; owner's
  call).
- **Phone Changes header:** at 360 "vs origin/main" is cut to "v…" (F note); the commit message field's text is cut
  under its "Commits all 2" label.
- **Grid at 390:** as F.
- **Forked review's Changes** still lists the parent's edit under `/private/tmp/qaG3/repos/app/` (E, F note).
- **Not run:** `qa/parity.mjs` and `qa/layout.mjs`; agent sessions don't run the outer-loop gates. The desktop-first
  how-to tabs are a site concern and were skipped.

## Cleanup

My two servers, both started by me, are stopped (`lsof … | xargs kill`, nothing left listening in 4900–4999):
- `wisp serve` :4961;
- `wisp-grid dev` :4967, which stopped its boxes.

No other process was touched. Data stays under `/tmp/qaG3` and `/tmp/qa-keep/G3`; shots are in `/tmp/qa-keep/shots-G`.
The shared `/tmp/wisp-target-fixer` was used as named and left in place.
