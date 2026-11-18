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
| 20 | Delegate button does nothing | not reproduced in Chrome, WebKit or Safari 18.6 with native clicks; recheck in the desktop app after install |
| 21 | Full auto pill does nothing | not reproduced (as 20); recheck in the desktop app |
| 22 | Profile circle does nothing | not reproduced (as 20); recheck in the desktop app |
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
| 32 | QA rounds until a round finds nothing, for desktop, phone and web (Grid) | done for desktop, phone and Grid: QA round 5 (aaee18de) found 0 new on all three, every finding of rounds 1-4 fixed and re-checked; left: the Grid landing's old launcher screenshots (rs-grid) |
| 33 | Phone New task: composer uses the full width; no scrollbar in the prompt | done (1ab36eb9) (prompt grows to 340 px as v1; scrollbars hidden on phones) |
| 34 | My Stuff matches v1 (opening a saved item, every flow) | done (1ab36eb9, 77c0966e): v1 cards, viewer, doc reader, apps inside wisp, full menu, keys, Undo, anchored Allow/Edit cards |
| 35 | Apps made in wisp match v1; parity reviewed by other agents | done (0f43f949: the last difference, code-card colour, fixed); independent review by QA r2 |
| 36 | Composer forgets the picked harness (back to Auto) | done (1ab36eb9) (server setting runtime.launch, as v1 config.defaults; needs the dev server rebuilt) |
| 37 | Disable Auto everywhere (keep the code, one switch) | done (1ab36eb9) (setting runtime.auto, default off; picker, Settings, ⌘K, crew editor, server defaults) |
| 38 | **Last**, when UI reviewers find nothing more: UI consultants for desktop, phone and web; research similar tools; fewer options on screen (submenus); focus on composer, task list, task page, right panel; I pick what to build | in progress (2026-11-18, the reviewers found nothing more): consultants gucci (desktop), ui-home (phone), port-grid (web Grid), research first, proposals in /tmp/qa-keep/ux-*.md; I pick what to build |
| 39 | Opening a task from search shows a blank page | done (1ab36eb9) (navigation resolves bare ids for every caller) |
| 40 | Desktop submenus open on hover | done (1ab36eb9: flyout with a safe triangle; touch drills in) |
| 41 | Transcript view does nothing on a running task | done (1ab36eb9) (v1 renderLiveTranscript: timeline so far above the working panel, no end row; seen on the mock peer) |
| 42 | Theme plugins never apply their partStyles | done (ui-shell, 2026-11-17: `plugins.styles` + `plugins.changed`, the shell's @layer plugins sheet, one allow-list with the kernel check; `wisp plugin trust`) |
| 43 | Long threads (wisp task d8ee0a207d52, plan docs/v2-long-threads.md): Notes in a Thread tab, the note before each follow-up, Fresh thread, health, compaction audit, output offload | done (8ee10b59, 77c0966e, dd1215c0, baf6cebd): Notes, wisp_notes, `wisp notes`, note before follow-ups, offload (Claude; Codex in Full auto), health, Thread tab (variant A), Fresh thread, compaction audit; left by ADR-0097: opencode offload once checkable |
| 44 | Keep this list and check it every job | done (this file) |

## Also found while fixing (not from the owner)

| Point | Status |
|---|---|
| Initial JS over budget | done (1ab36eb9: 117.1 KB; again 120.3 → 116.5 KB in 7f2416b3: logos as files, ship split) |
| PR pane footer says "Checked time.now" (untranslated key) | done (1ab36eb9) (PrPane reads the kit words under `kit.`) |
| Machines: the Mac's installed v1 app shows as a second machine | needs a machine id at registration; goes away once v2 replaces v1 |
| Home's "Sign in to GitHub" tip shows even when gh is signed in | done (dd1215c0) |
| WebKit "ResizeObserver loop" message | done (dd1215c0, 0f43f949: VirtualList, Toolbar, Diagnostics) |
| Server budget: write amplification 44.5× > 44× | done (7f2416b3: 39.8×, ADR-0100) |

## Then

Push, CI green on every OS, merge PR #756, release through release.yml, install the release on this Mac and check it runs.
