# Audit: the task side panel (2026-11-19)

Owner, 2026-11-19, with a screenshot (the panel open on Shell, one unlabelled icon tab, a big empty terminal area under a
short session, `+` and `⋯` in the head): "run audit on this panel, too many bugs, and also make it by default show the
tabs for PR, changes etc". Decision recorded in docs/v2-plan.md §3 (UI) and docs/owner-list.md row 53: the strip shows
v1's tabs by default, labelled (Changes, PR, Shell, Preview; Watch and pinned tabs when they apply), icons only when
the panel has no room; counts stay. Replaces UX 2026-11-18 desktop #5's "tabs only with content, the rest behind +".

Code: `plugins/shell-task/ui/src/side/` (SidePanel, ChangesPane, PrPane, ShellPane, PreviewPane, WatchPane, strip.ts),
the kit's `Tabs` (`packages/ui/src/controls/Tabs.tsx`) and `app-frame.css`. v1: `web/js/features/panel.js`,
`changes.js`, `server/routes/workspace.js`; v1 look `design/v1-reference/task.changes*`.

## How it was run

- **Mock peer** (`npx vite --port 5191` in apps/web, `POST /__mock/reset?seed=ask`): the task with a PR (#812, checks
  passed), two edited files, a pinned My Stuff app and the Thread tab. Scripted with Playwright
  (`/tmp/qa-keep/side/pw/audit.mjs`, `more.mjs`, `phone.mjs`): Chrome and WebKit, 1440 and 1280, light and dark; the
  phone sheet at 390 with the iOS and Android looks (the fake Capacitor bridge, as qa/phone.mjs).
- **Dev UI** (http://localhost:5180, the real `wisp serve` on ~/.wisp-dev, never restarted): a task whose worktree was
  cleaned up after it finished (most finished worktree tasks there), a task in place in the shared checkout (read only:
  nothing staged, committed or merged), a scratch-folder task. No paid runs; Merge only opened and cancelled.
- Shots before and after: `/tmp/qa-keep/side/` (`before-*`, `after-*`).

## Results

| Action | Desktop Chrome | Desktop WebKit | Phone iOS / Android | Before | After |
|---|---|---|---|---|---|
| `]` opens, `]` and × close | pass | pass | n/a (sheet) | pass | pass |
| Strip shows Changes, PR, Shell, Preview, labelled | fail → pass | fail → pass | pass | **fail** (#1, #2, #3) | pass: all labelled at 1440; with two pinned tabs and a count the last ones go to their icon (Preview at 1440, all but the open tab and Changes at 1280's 396 px); never clipped; the open tab always labelled |
| No `+` menu (every tab in the strip) | fail → pass | fail → pass | pass | **fail** (#1) | pass |
| Each tab opens (Changes, PR, Shell, Preview, pinned app, Thread) | pass | pass | pass | Preview only behind `+` | pass |
| `←` / `→` walk the tabs | fail → pass | fail → pass | n/a | **fail** (#4) | pass |
| Changes count, PR `#812 ✓` on their tabs | pass | pass | pass | pass | pass |
| `⋯`: Copy path, Copy branch, Open in editor, Open in terminal app | pass | pass (clipboard not readable by the script) | n/a | pass | pass |
| Resize by the grip (1440) | pass | pass | n/a | pass | pass |
| Resize at 1280 | blocked by design | blocked by design | n/a | the task column keeps its minimum (pane-widths.ts) | same |
| Changes: list, fold a diff | pass | pass | pass | pass | pass |
| Changes on a task in place | — | — | — | **fail** (#6: 198 files, the shared checkout's) | the edits wisp saw |
| Changes when the worktree is gone | — | — | — | **fail** (#7: "No changes yet" + a raw toast) | "This task's folder is gone" |
| Changes: Open file… | pass on git | pass on git | n/a | **fail** where git can't list (#8) | offered on git's diff only |
| No toast from the panel's background reads | — | — | pass | **fail** (#5) | pass |
| PR: card, checks, Merge asks first (cancelled) | pass | pass | pass | pass | pass |
| Shell: start, type, Start again | pass | pass | pass | pass | pass |
| Shell: End shell asks first (v1) | fail → pass | fail → pass | fail → pass | **fail** (#9) | pass |
| Shell: a reattached shell fits the pane | — | — | — | **fail** (#10: prompt cut at the edge) | pass |
| Shell: the area under a short session | — | — | — | **fail** (#11: a dark box in a white margin) | v1's edge to edge |
| Preview: not running says how to run; Run starts it | pass | pass | pass | pass | pass |
| Pinned My Stuff tab opens its app | pass | pass | pass | pass | pass |
| Switch task with the panel open | pass | pass | n/a | pass | pass |
| Task head's Changes `+n −n` on a task in place | — | — | — | **fail** (#12: +1962 of the checkout) | worktree only |
| Page errors | none | none | none | none | none |

## Findings

1. **The strip hid v1's tabs behind `+`** (the owner's screenshot): only tabs with something in them showed, so a
   fresh task's panel had one tab (Shell). Fixed: `strip.ts` `sideStrip` gives v1 panel.js paintName's fixed order
   (a terminal task's CLI, Changes, PR, Shell, Preview, Watch while there's one, then pinned tabs), the `+` menu is gone.
   Test: `plugins/shell-task/ui/test/side-strip.test.ts` (`the_strip_shows_changes_pr_shell_and_preview_even_when_they_have_nothing_yet`, `a_terminal_task_leads_with_…`).
2. **Labels hidden whenever the panel was under 420 px**, whether they fit or not (v1's container query): a 1280
   window gives the panel 396 px, so every tab was an unlabelled icon. Fixed: the kit's `Tabs fit` measures the row
   (ResizeObserver, no per-frame work) and turns tabs to their icon only while it overflows, from the end of the strip
   backwards (pinned tabs first, then Preview…; the open tab keeps its label), re-measured when the tabs change, and back
   as room comes (design/layout.md priorities). Test:
   `packages/ui/test/controls-tabs.test.tsx` `fit_turns_the_last_tabs_to_icons_one_at_a_time_…`.
3. **A pinned tab's long label squeezed the panel's own** to "Cha…", "#…", "S…" beside a whole "Token budget dash…".
   Fixed by #2: the panel's tabs keep their labels, a pinned tab goes to its icon first.
4. **`→` moved one tab, then the focus fell to the page**: the panel makes its tab items afresh on each change and the
   kit rebuilt every button. Fixed: the kit's Tabs keys its buttons by value. Test:
   `controls-tabs.test.tsx` `arrow_keys_keep_walking_when_the_items_are_made_afresh_on_each_change`.
5. **Background reads toasted raw errors on desktop**: opening the panel on a task whose worktree was cleaned up
   showed "no such folder: /Users/mokh/.wisp-dev/worktrees/wisp-01a1f…" on every open (the Changes count's `git.changes`,
   the Changes pane's own read). Fixed: both reads are quiet everywhere (they were on phones only); the pane says it in
   words (#7).
6. **Changes on a task in place diffed the whole project folder**: in the shared checkout it listed 198 files, every
   session's and the user's own work, and offered to commit them. v1's GET /sessions/:id/changes answered `files: null`
   without a worktree and the page showed the edits wisp saw. Fixed: git diffs the worktree only (`changes.ts`
   `worktreeDir`), otherwise "edits wisp saw". Test: `changes.test.ts` `git_reads_a_worktree_only_…`.
7. **Changes on a task whose worktree is gone said "No changes yet"** (plus #5's toast). Fixed: the refusal is shown in
   words on desktop too: "This task's folder is gone · wisp cleaned it up after the task finished…", or the edits wisp
   saw when it saw some (v1 "The worktree was removed after the merge: showing the edits wisp saw").
8. **Open file… on a folder git can't list closed the palette at once and left the focus on the page**, so the next
   keys ran as shortcuts (typing "peers" marked the task done with `e`). v1 offered Open file… on git's diff only.
   Fixed: shown only when Changes is git's diff, and a refused list keeps the palette open (what's typed stays in its
   field) and says why once. Test: `open-file.test.ts` `a_refused_file_list_keeps_the_palette_open_and_says_why`.
9. **End shell ended at once**; v1 asked "End the shell? Whatever runs in it stops with it." Fixed: an alert dialog,
   End shell in the danger style (the terminal task's End terminal too).
10. **A shell reattached at another width kept its old size**: the prompt ran past the pane's right edge and was cut
    (the real app, a shell started in a wider window). Fixed: on the shell's hello the pane sends its measured size when
    it differs. Test: `plugins/shell-task/ui/test/shell-size.test.ts`.
11. **The Shell sat in a dark box inside a white margin**, which read as a big empty area under a short session; v1's
    `.pn-shell` is the terminal to the panel's edges with its ended row on the terminal's colours. Fixed (browser look;
    the phone looks keep theirs): the Shell body is flush, its screen 8px 10px, its foot v1's `.pn-ended` row.
12. **The task head's Changes chip had the same cause as #6**: +1962 −0 on a task in place that edited nothing, and it
    opens this pane. Fixed: the head reads git for a worktree only (packages/shell TaskView.tsx). The audit-A #9 test
    keeps covering git's figures: the mock's "Add rate limiting" task now has its worktree (seed.ts), as a real one would.
13. **Mock only**: the PR pane said "no pull request" on the mock (`ship.details` looked the row up by the bare id) and
    the edits wisp saw never matched the mock's turns (keyed `local:id`). Fixed in mock-commands.ts and `changes.ts`
    `ofTask` (accepts both). Test: `changes.test.ts` `a_turn_belongs_to_its_task_…`.

Not bugs: WebKit's clipboard can't be read by the script (Copy branch shows its toast); at 1280 the panel can't be
widened past 396 px with the list open because the task column keeps its minimum (pane-widths.ts, as designed); a deep
link `?side=` on a task outside the loaded list showed no panel once and could not be reproduced.

## Follow-up (2026-11-19, after bb030486): the strip at 1280

In the dev app at 1280×860 (panel 396 px, 297 px for the tabs) the strip showed only Shell labelled. Two causes:

- **The fit over-stepped.** It stepped one tab at a time and measured again, but its first check ran while Solid was
  mounting, so no step reached the DOM before the next measure: every measure saw the same overflowing row and it ran
  to the last level, keeping widths from that stale row, so it never came back. Fixed in the kit's Tabs: one measuring
  pass (each tab labelled and as its icon, at v1's padding and tight, read by setting the row's own attributes for a
  moment before the browser paints) and a pure plan, `fitPlan`.
- **v1's padding can't hold four labels in 297 px** (v1 itself went icons-only under 420 px). The plan's order: minor
  tabs (pinned, Thread, Watch) to their icon first at v1's padding; then the row tightens (5px tab padding, 4px icon gap,
  1px between tabs, 28px icon tabs); then Preview, then the rest; the open tab keeps its label. The row also counts its
  -8px pull as room.

Measured after, Chrome and WebKit, light and dark: at 1280 Changes 79.6, PR 43.4, Shell 56.4, Preview 74.2 labelled,
Thread its 28px icon (290 of 305 px); at 1440 all five labelled at v1's padding (380 of 389 px). Tests:
`packages/ui/test/controls-tabs.test.tsx` `fit_at_396px_keeps_changes_pr_shell_and_preview_labelled_thread_gives_way_first`,
`fit_gives_way_from_the_end_one_tab_at_a_time_preview_before_the_rest_the_open_tab_last_of_all`.
