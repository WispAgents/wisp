# The owner's list

Every point the owner raised for wisp v2 since the desktop test on 2026-11-17, with where it stands. **Read this at the
start of every job and update it in the same commit as the work.** Nothing leaves this list until it's done and seen
in the real app (or the owner drops it). Statuses: `done` (committed and checked), `in progress (who)`, `todo`,
`not a bug` (with why).

## Desktop test, 2026-11-17

| # | Point | Status |
|---|---|---|
| 1 | Horizontal scrollbar under the task list | done (3c0007ee) |
| 2 | Themes don't switch | done (743fc711: theme is a device setting) |
| 3 | Theme setting shown twice | done (743fc711) |
| 4 | Icon touches text (file chip) | done (80fa6b0d) |
| 5 | Tool output runs past the block | done (80fa6b0d) |
| 6 | Working card: glow not aligned with "is working" | done (80fa6b0d) |
| 7 | Missing harness icons in the picker | done (80fa6b0d) |
| 8 | Held harness-changed card: raw label, Start now loops | done (45f288d7) |
| 9 | Worktree removal dialog looks bad | done (80fa6b0d) |
| 10 | Home Needs you opens a blank page | done (3c0007ee; root fix in navigation, #39) |
| 11 | A 👍 re-renders the report | done (3c0007ee) |
| 12 | Save for later: toast, nothing changes | done (743fc711) |
| 13 | Usage-limit card spacing | done (80fa6b0d) |
| 14 | This computer card shows its grid URL | done (45f288d7) |
| 15 | Asleep / Wake spacing | done (3c0007ee) |
| 16 | Menus start with a separator | done (3c0007ee) |
| 17 | Sidebar view button does nothing | done (743fc711) |
| 18 | Can't resize the list or the right panel | done (743fc711) |
| 19 | Issue picker loading: one skeleton row | done (80fa6b0d) |
| 20 | Delegate button does nothing | not reproduced in Chrome, WebKit or Safari 18.6 with native clicks; recheck in the desktop app after install; likely the same cause as #45 (the dev app died under the open window) |
| 21 | Full auto pill does nothing | not reproduced (as 20); recheck in the desktop app; likely the same cause as #45 (the dev app died under the open window) |
| 22 | Profile circle does nothing | not reproduced (as 20); recheck in the desktop app; likely the same cause as #45 (the dev app died under the open window) |
| 23 | Crew tint doesn't change | done (55dccadf) |
| 24 | Make a pane rerun forgets it's a pane | done (45f288d7) |
| 25 | Make a pane: agent never told how a wisp pane is made | done (4bef036f) |
| 26 | Opening a My Stuff item crashed the desktop app | done in code (45f288d7); check in a rebuilt desktop app |
| 27 | Delegate modal doesn't look like v1 | done (b044366e; presets 639f96a1) |
| 28 | Held task says Finished on Home / phone header | done (b044366e) |
| 29 | After a restart, Trust holds again | done (b044366e; Windows test fixed, uncommitted) |
| 30 | Pin reverts | done (b044366e) |

## Later the same evening

| # | Point | Status |
|---|---|---|
| 31 | QA round 1: 32 findings (/tmp/qa-keep/qa-round1.md) | fixed except #16 ResizeObserver (benign browser message) and #31 (v1 layout); committed 1ab36eb9 |
| 32 | QA rounds until a round finds nothing, for desktop, phone and web (Grid) | in progress: rounds 1-11 fixed (latest c0cd7cd9, f8a68826); round 11 found 0 on phone and Grid, 1 minor on desktop, 2 in the CLI (fixed); round 12 on all three running |
| 33 | Phone New task: composer uses the full width; no scrollbar in the prompt | done (1ab36eb9) (prompt grows to 340 px as v1; scrollbars hidden on phones) |
| 34 | My Stuff matches v1 (opening a saved item, every flow) | done (1ab36eb9, 77c0966e): v1 cards, viewer, doc reader, apps inside wisp, full menu, keys, Undo, anchored Allow/Edit cards |
| 35 | Apps made in wisp match v1; parity reviewed by other agents | done (0f43f949: the last difference, code-card colour, fixed); independent review by QA r2 |
| 36 | Composer forgets the picked harness (back to Auto) | done (1ab36eb9) (server setting runtime.launch, as v1 config.defaults; needs the dev server rebuilt) |
| 37 | Disable Auto everywhere (keep the code, one switch) | done (1ab36eb9) (setting runtime.auto, default off; picker, Settings, ⌘K, crew editor, server defaults) |
| 38 | **Last**, when UI reviewers find nothing more: UI consultants for desktop, phone and web; research similar tools; fewer options on screen (submenus); focus on composer, task list, task page, right panel; I pick what to build | done (b6bc0570, 3c9fc488 and fixes in rounds 6-11): desktop 1-9, 11, 13; phone 1-14; Grid 1-11; not now: desktop 10, 12 |
| 39 | Opening a task from search shows a blank page | done (1ab36eb9) (navigation resolves bare ids for every caller) |
| 40 | Desktop submenus open on hover | done (1ab36eb9: flyout with a safe triangle; touch drills in) |
| 41 | Transcript view does nothing on a running task | done (1ab36eb9) (v1 renderLiveTranscript: timeline so far above the working panel, no end row; seen on the mock peer) |
| 42 | Theme plugins never apply their partStyles | done (ui-shell, 2026-11-17: `plugins.styles` + `plugins.changed`, the shell's @layer plugins sheet, one allow-list with the kernel check; `wisp plugin trust`) |
| 43 | Long threads (wisp task d8ee0a207d52, plan docs/v2-long-threads.md): Notes in a Thread tab, the note before each follow-up, Fresh thread, health, compaction audit, output offload | done (8ee10b59, 77c0966e, dd1215c0, baf6cebd): Notes, wisp_notes, `wisp notes`, note before follow-ups, offload (Claude; Codex in Full auto), health, Thread tab (variant A), Fresh thread, compaction audit; left by ADR-0097: opencode offload once checkable |
| 44 | Keep this list and check it every job | done (this file) |
| 45 | The project selector does nothing when clicked (owner, 2026-11-18) | cause found 2026-11-18: not the selector (it opens in Chrome and WebKit, both kinds); the dev app (Vite, wisp serve, the desktop window) ran as a child of the lead's shell and died at each session restart, so the open window's clicks went nowhere. The dev app now runs in its own session (/tmp/devrestart.sh, setsid); recheck in the restarted window |
| 46 | A crew member's task row: its title and details don't line up with a normal task's (dot) rows (owner, 2026-11-18) | done: the face sits centred on the dot's spot (18 px over the 7 px dot, as v1's crew dot), titles and details line up at x 100 on every row, 7 px gap (Chrome, WebKit) |
| 47 | Desktop: on the composer page (Home / New task) the window can't be dragged from the top; the task page can (owner, 2026-11-18) | done: a drag strip in Home's top padding on desktop and in the browser (not on phones); pressing it asks the window to move (start_drag), as the task head does |
| 48 | Separate parallel agents check performance and find problems; fix until a round finds nothing new (owner, 2026-11-18) | in progress: server round 1 fixed 9/10 (an answer cache bounded, CI bench lock check, worktrees/machine/startup/list/share/health/ls paths; #6 list paging not now: the list goes compressed, 4.9 MB → 224 KB); UI round 1: style recalcs −67–83 %, open task 369 → 123 ms, side panel 359 → 60 ms, ⌘K 240 → 69 ms; round 2 (this commit): server 8/8 fixed (patch p99 after a client burst 273 → 105 ms, blame 7×, ls newest 50); UI ⌘K 940 → 61 ms, submenu hover 255 → 29 ms, picker reopen 0 requests, Settings 2nd visit 163 → 47 ms; first-screen gate landed as a ratchet (ADR-0105: 173/167 KB JS, 30/40.5 CSS, target 120/30); left: task open 150 ms at 4×, row rebinding on scroll, heap while streaming, the title shimmer's recalcs |
| 49 | Settings → Agents: many bugs; adding something is slow; signing in to other accounts expected; thorough audit (owner, 2026-11-18) | done (c0d655ad): account rows had the wrong id (every account control dead), sign-in progress live, status checks in parallel, Add another account as v1, gh setup-git |
| 50 | Be more thorough: check every action in the app end to end, especially orchestration and agent setup (owner, 2026-11-18) | audit A's ~60 failing actions fixed (c0d655ad): Restore only in the task's folder, merge keeps unsaved worktrees, fork/rerun/compare get their own worktrees, attachments and the team reach the agent, /side apart, automations edit/pause/delete, setup F1-F33 (one not reproduced), phone swipes run; audit B (2026-11-19, on c0d655ad): tasks 10, orchestration 19, setup 13 findings, all fixed (this commit; three #19 smells left on purpose: a WebKit RO not reproduced, v1's Needs you count kept, MCP listing per ADR-0107); audit C (2026-11-19, on 928fb5f3): tasks 5 new + 1 left from B, all fixed (this commit); orchestration C: 18 new (4 major: wisp_launch drops worktree/project, quick follow-ups lost to a SQL race, check-ins/triggers make a worktree each, In place turns worktrees off globally) + 4 left from B, all fixed; setup: 9 new, all fixed (this commit); audit D next |
| 51 | Settings → Agents sign-in looks unstyled (long OAuth link, Codex code shown twice); Antigravity Sign in errors (owner, 2026-11-19) | done: v1's sign-in box in the kit (SettingsSignIn), one panel per account, agy signs in under a pty on macOS, never-finished accounts read Signed out; tested on the real dev app with the owner's accounts |
| 52 | Check Remove account and Sign out (owner, 2026-11-19) | done: Remove asks, then the row goes and its folder and running sign-in go (real app, a throwaway Codex account); Sign out answers in 3 ms, sticks when the CLI agrees, and says why when it doesn't (agy). Fixed: Sign out during a running sign-in left it running. Your real Claude/Cursor/Grok logins were not signed out: tested with fakes |
| 54 | Links from the desktop app don't open in the browser (owner, 2026-11-19) | done: orphaned headless QA Chromes had become the front Chrome and swallowed every URL open (stopped); links now open inside the click (04cd03b7). QA scripts must close their browser and never use perl alarm |
| 53 | The task side panel: too many bugs, audit it; show the tabs (Changes, PR, Shell, Preview …) by default, labelled, as v1 (owner, 2026-11-19, screenshot: Shell open, one unlabelled tab, a big empty terminal area) | done (this commit): v1's tabs labelled by default (Changes, PR, Shell, Preview), icons only when out of room; 12 bugs fixed (docs/qa/audit-side-panel.md) |
| 55 | Website and docs: never say v2; every feature from CLI, TUI and apps; desktop + phone apps as headline features; phone app waitlist by email; show why wisp is the new way to build with agents; all Linear guides (TRY-8/9/10/20/25) + more; great SEO, responsive, fast (owner, 2026-11-19) | done (this commit): new landing and Get the app page, ~25 guides with App/TUI/CLI tabs incl. TRY-8/9/10/20/25, phone waitlist on the Grid, measured numbers only, SEO tags, 0 broken links, phone first paint 980 → 308 ms; Pages publishes only the built site (internal notes were public). Opened for the owner once the desktop self-update lands |
| 56 | The desktop app can't update itself: Update fails with "updates itself when it restarts" and stays on the old version; fix it like v1's fetchApp, check detect against Tauri's layout, F-upd-1 partial until seen end to end (owner, 2026-11-19) | in progress (fixer): download, sha256, stage per OS and relaunch wired with tests (plugins/update/host/src/app.rs, tests/app.rs); detect knows `resources/app`; the relaunch waits for the real shell pid; reviewer-r177's 5 fixes (Windows helper leaves the shell's job, Linux waits for the old shell, no digest refused, macOS sets the old bundle aside beside itself, sh paths single-quoted) and `WISP_UPDATE_FEED` for a local fake feed; not yet seen end to end in the packaged app |
| 57 | Site: show the desktop app's way when the visitor is on a computer (owner, 2026-11-19) | done (this commit): Desktop app · Phone app · TUI · CLI tabs; Desktop first on computers with real desktop screenshots, Phone first on phones, remembered across the site and docs |
| 58 | Docs: clicking sidebar items does nothing (owner, 2026-11-19) | done (this commit): every sidebar group was empty (Starlight matched pages by a src/content/docs path ours never has); fixed, and the docs build now fails on an empty group |
| 59 | After this round, one subagent at a time (owner, 2026-11-19) | in effect: audit F's 24 fixed by one coder (this commit) |
| 60 | Formulas: declare goals by name and reference them from tasks; a `default:` under `tasks:` (and `goals:`) gives defaults to all of them (owner, 2026-11-19) | done (this change, ADR-0110): `goals:` declares goals by name with `goals.default` (the singleton `goal:` gone), `tasks.default` under every declared task's run, a task's `goal: <name>` checked by `wisp validate`; `wisp "…" --goal <name>` / `--goal-text`, `wisp goal set <task> <name>`, `wisp_launch`'s `goal` (both MCP servers), Goal… in New task and the reply box lists the declared goals; guides (software factory §4, Goals and judges) and the kinds reference updated |
| 61 | Run with a formula everywhere: `wisp "prompt" --formula f.yml` (plain launch), `wisp run <declared-task> --formula f.yml` by name without applying, task files that reference formula files/modules (`formula: ./wisp.yml`) with names resolved against them (agent, goal…), the app's New task opens a task file or picks a formula, MCP launch takes `formula`; and everything atomic and referenced: every kind (agents, skills, connectors, MCP servers, projects, goals, tasks, prompts, crew, teams, triggers, schedules, recipes, channels, budgets, settings) declarable on its own with an address, every field that names another thing takes a reference (inline still allowed), resolved the same in formulas, task files, CLI flags, MCP and the app; validate refuses unknown/circular refs naming what exists — starts with an audit of every kind and field (owner, 2026-11-19) | done (this commit, ADR-0111/0112): references audit (docs/qa/refs-audit.md), every name field checked, --formula everywhere, task files reference formulas/modules; three security reviews closed: agents' formulas read only their task's folder, no secrets/commands, a child never gets more than its parent (one check in runtime.create), agents never raise a mode or change the user's setup |
| 62 | Menus on phones: submenus as a drill-in sheet, never a side flyout off the screen (owner, 2026-11-19, screenshot) | done (uncommitted, coder-r193): the browser on a touch or <720 px screen opens every kit menu as a bottom sheet (PhoneMenu in the web look) that drills into submenus under a ‹ back row; desktop keeps flyouts; phone apps keep their native menus; packages/ui/test/menu-sheet.test.tsx; checked in Chrome and WebKit at 390/360, web touch and mouse, iOS/Android stubs, light and dark |
| 63 | Many goals (and recipes) in the Goal… menu (owner, 2026-11-19) | done (uncommitted, 2026-11-20): recent in this project first then A–Z; past 7 a filter box (name and description, Enter picks the first match), the first 7, All goals… (searchable picker: dialog on desktop, sheet on touch/narrow), New goal… last; Recipes the same; `/goal ` suggests declared goals; `--goal <TAB>` completes in bash, zsh, fish. Seen in Chrome and WebKit at 1440 and 390, light and dark (mock, 30 goals) |
| 64 | The CLI knows the terminal's context so a user can ask there ("check the error above") (owner, 2026-11-19) | built (ADR-0023, wisp.termctx, unit tests) but missing from the docs and never checked by a QA round: add a guide (+ landing mention) and cover it in the next audit (tmux, iTerm, Apple Terminal, VS Code, the shell hook) |
| 65 | Factories: a user declares a software factory and runs a task through it instead of one agent (owner, 2026-11-20; researched: stations spec→build→checks→review→ship, deterministic gates, human approval at spec and merge, loop back on failure with a limit, durable handoffs) | to do after row 61 lands: kind `factories` (atomic, referenced; built-in `default`), stations (agent/agents, run: commands as gates, approve/merge: human, worktree, parallel/compare), `loop: { back_to, max, then: ask }`; run with `wisp "…" --factory <name>`, the app's picker (One agent / a factory), MCP `factory`, `factory:` in tasks/triggers/crew/task files; one row with stations as steps; built on workflows, named goals, review, ship, holds, budgets; ADR + design check first |
| 66 | (from row 61 review) An agent's setup changes (crew, connectors, projects: v1's wisp_crew_add/edit/remove, wisp_connector_add, projectAdd) ask the user with a card (what, the command it would run, Allow/Deny) instead of being refused; tests never read the real Cursor/opencode MCP config | to do next |

## Also found while fixing (not from the owner)

| Point | Status |
|---|---|
| Initial JS over budget | done (1ab36eb9: 117.1 KB; again 120.3 → 116.5 KB in 7f2416b3: logos as files, ship split) |
| PR pane footer says "Checked time.now" (untranslated key) | done (1ab36eb9) (PrPane reads the kit words under `kit.`) |
| Machines: the Mac's installed v1 app shows as a second machine | needs a machine id at registration; goes away once v2 replaces v1 |
| Home's "Sign in to GitHub" tip shows even when gh is signed in | done (dd1215c0) |
| WebKit "ResizeObserver loop" message | done (dd1215c0, 0f43f949: VirtualList, Toolbar, Diagnostics) |
| Server budget: write amplification 44.5× > 44× | done (7f2416b3: 39.8×, ADR-0100) |
| A submenu's items that arrive while it's open (a plugin's later part landing) did nothing when clicked: Task options › Add to crew… in the first second; the test passed after a 4 s wait and timed out on Windows CI | done: the kit's menu keys rows by value, so an open flyout keeps its machine (packages/ui/test/menu.test.tsx an_item_added_to_an_open_submenu_can_be_chosen_*) |
| CI on c0d655ad: Rust targets red (serve_history MCP list, serve_worktree_safety git identity and CRLF, delete_race's fake missed the /btw hang cue, handoff /btw prompt, serve_cli_project path) and a real Windows bug: Bring in skipped stray worktrees (git's C:/ vs wisp's C:\ compared as strings) | done (fixer): tests updated for deliberate changes; Bring in compares folders (plugins/worktrees/core/src/offer.rs) |

## Then

Push, CI green on every OS, merge PR #756, release through release.yml, install the release on this Mac and check it runs.
