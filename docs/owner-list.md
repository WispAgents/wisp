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
| 50 | Be more thorough: check every action in the app end to end, especially orchestration and agent setup (owner, 2026-11-18) | audit A's ~60 failing actions fixed (c0d655ad): Restore only in the task's folder, merge keeps unsaved worktrees, fork/rerun/compare get their own worktrees, attachments and the team reach the agent, /side apart, automations edit/pause/delete, setup F1-F33 (one not reproduced), phone swipes run; audit B (2026-11-19, on c0d655ad): tasks 10, orchestration 19, setup 13 findings, all fixed (this commit; three #19 smells left on purpose: a WebKit RO not reproduced, v1's Needs you count kept, MCP listing per ADR-0107); audit C next |

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
