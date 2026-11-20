# Audit G, tasks: fixes (worktree at cd948e6a, uncommitted), 2026-11-20

One row per finding of `docs/qa/audit-G-tasks.md`: its fix, the test that covers it, and the proof in real Chrome (and
WebKit where named).
- **Seen failing** means the test was run on the old code (the old file put back for the run) and failed.
- **Pure, new** means the test covers a helper the fix added; the audit's own repro is the before, and the live run is
  the after.

**Scratch wisp.** `/tmp/qaH3` is a copy of audit G's seeded `/tmp/qaG3`. The name has the same length, so the store's
absolute paths were rewritten in place: `PRAGMA integrity_check` ok.
- `wisp serve --port 4962`, with the fake harnesses, `WISP_HOME=/tmp/qaH3/wisphome` and `HOME=/tmp/qaH3/home`.
- Fresh setup cards (`hsetup1-3`) were seeded by `/tmp/qa-keep/H/seedH.mjs`. Rewriting the wrapper's paths changed its
  bytes, so those runs were held first and started with `wisp held … start`.
- The UI is a `vite build` of this worktree in `/tmp/qaH3/web`.
- Playwright with system Chrome and WebKit:
  - desktop: the Tauri stub, 1280 and 1440;
  - phones: the Capacitor stub, iOS 390/360 and Android 360;
  - the phone-width browser: 390, no stub.
- Scripts are in `/tmp/qa-keep/H/h-*.mjs`; shots are in `/tmp/qa-keep/shots-H/`. Every browser is closed in `finally`.

## NEW findings

| # | Fix | Test | Proof (live) |
|---|---|---|---|
| 1 [major] first ⋯ sheet titled with another task | `controls/Menu.tsx`: a press that comes before the sheet's machine has loaded is replayed on the pressed element when the machine is the phone overlays' (phone apps, or the browser on a sheet screen), not on the list area. The sheet read its title from the area's first row | `ui lazy-sheet-title.test.tsx the_first_sheet_after_a_load_is_titled_by_the_pressed_row_not_the_lists_first_audit_g_tasks_1` (**seen failing**: "Filler 12") | C, W Web 390, a fresh browser each: top row "Filler 12"; tapped "Setup H project" → sheet "TOPIC[Setup H project] Add a project"; tapped "Setup H long" → its own title |
| 2 [major] phone apps' New task: no declared goals, no formula | `HomeView.tsx`: the phone's More gets desktop's **Goal…** act (declared goals, filter, All goals…, New goal…; while a goal is on, the toggle that turns it off) and **Formula or task file…**, with a ✕ chip. `launcher-fold.ts`: a folded submenu keeps its `filter`, a goal its filter finds is taken, and More warms the goals (`onWarm`) | `shell phone-bar.test.ts the_phone_more_offers_goal_with_the_declared_goals_and_its_filter_audit_g_tasks_2` (folds the real `goalAct` with 32 goals; the old fold dropped the filter) | C iOS 390 L, And 360 D: ⋯ = "Goal… · Worktree · Start later · Formula or task file… · Compare… · Start from an issue · Recipes"; Goal… shows goal-01…07, "All goals (32)…", "New goal…"; the filter "goal-2" narrows to goal-20…29; All goals… opens the picker. Shots `h-phpick-more-*`, `h-phpick-goal-*`, `h-phpick-goalfilter-*` |
| 3 [major] → stops at Shell, then goes into the shell | `SidePanel.tsx` records the last key in the panel (a pointer press clears it). An arrow, Home or End that selects Shell or a takeover calls `shellWalkedTo()`. `ShellPane.tsx`: that pane starts its shell but takes no focus, from its mount or its start. Enter, Space or a click on the tab (the document click handler), and Start again, still focus the terminal | `shell-task shell-focus.test.tsx an_arrow_key_onto_shell_starts_it_but_leaves_the_keys_on_the_tab_audit_g_tasks_3` (**seen failing**) | C and W D 1280 and 1440: → walk "PR #1 → Shell → Preview → Counter app → Thread → Changes 1 → PR #1 → Shell", fb 3–21. `terminal.input` while walking was resizes only (`size 48x45`), no keys. Enter on Shell → "Terminal input"; typing goes out as `terminal.input`. A click on Shell still focuses the terminal (F #1). Shot `h-arrow-*` |
| 4 [minor] phone answer buttons 28/32 px | `platform/ios.css`, `android.css`: `[data-part~='task.ask'] [data-kit='wrap'] > [data-kit='button'] { min-height: var(--tap-min) }`, as the held card got (F #4) | `ui phone-answer-target.test.ts the_{ios,android}_look_gives_a_cards_answer_buttons_the_touch_target_audit_g_tasks_4` | C iOS 390: Show all 66×44, Allow 55×44, Deny 47×44, Approve 73×44, Stop 50×44, One more round 122×44. And 360: Allow 60×48, Show all 80×48, Approve 80×48, One more round 132×48. iOS 360: 44. Shots `h-setup-*-ios-390-light`, `h-fac-*-android-360-dark` |
| 5 [minor] long words run out of a card | `app-task.css`: `[data-kit='ask-text']` (the prompt) gets `overflow-wrap: anywhere`. `AskCard.tsx`: the question text gets `breakAnywhere` (`task.ask.question`) | `ui ask-text-wrap.test.ts …audit_g_tasks_5`; `shell-task setup-card.test.tsx a_question_quoting_a_long_path_breaks_inside_its_card_audit_g_tasks_5` | C iOS 390: the prompt's `"cwd":"/tmp/qaH3/…` ran to x 408 of 352 (`ask-text` `ow=normal`) and now wraps. The factory question ends at 359 of a card ending at 366 (iOS 390), 329 of 336 (And 360, iOS 360). Shot `h-fac-fac2-android-360-dark` |
| 6 [minor] factory reply box foot cut on phones | `run-factory.ts factoryFoot`: on a phone, a live run's hint is "Stop the run" alone, with the short placeholder "Reply on a station's task to add to it". `Task.tsx`: a hint with no words draws the link alone (`data-bare`); `app-task.css` stops it shrinking and drops the words' size container, whose `inline-size` made its min-content 0. Desktop keeps the sentence and the words | `shell factory-pick.test.ts a_phones_factory_reply_box_keeps_stop_whole_and_a_short_placeholder_audit_g_tasks_6` | C iOS 390, And 360, iOS 360: "Stop the run" 69/69 (sw/cw), the placeholder 231 px of 320/292. Desktop 1440: "Its stations do the work · Stop the run" and the full placeholder |
| 7 [minor] pill says "Factory · Default" | `factory-pick.ts factoryPick`: the pick carries `modelLabel: <name>`, so the bar reads "Factory · <name>" | `shell factory-pick.test.ts the_pill_names_the_picked_factory_audit_g_tasks_7` (the old pick gives "Factory · Default") | C, W D 1440: the Factory row → "Factory · default"; strict (fb 2–6) → "Factory · strict"; reopened, strict is picked. Shot `h-facpick-*` |
| 8 [minor] factory row shows Claude Code's logo | `row.ts rowMark`: a row with `factory` gets the factory's mark (`queue`, `list.row.factory`), never a harness logo. `RowParts.tsx HarnessMark` draws it | `shell-list row-two-lines.test.ts a_factory_runs_row_shows_the_factory_mark_never_a_harness_logo_audit_g_tasks_8` (pure, new) | C D 1440, W D 1280, C iOS 390: Factory default and Factory strict rows have the mark and no `ir-logo`. Shots `h-row-factory-*` |
| 9 [minor] Show all: raw JSON, stray "(Show all)", offered when nothing is cut | `runtime-core setup.rs`: a cut line ends with "…" alone. `full` is the same lines uncut, plus each kit file's body, and is empty when nothing was cut and there are no bodies. `AskCard.tsx`: Show all swaps the cut lines for the whole ones, in plain text, not a code block | `wisp-runtime-core setup::tests::show_all_is_the_whole_lines_in_words_and_only_when_something_is_cut_audit_g_tasks_9` (**seen failing**); `shell-task setup-card.test.tsx show_all_swaps_the_cut_lines_for_the_whole_ones…_audit_g_tasks_9` | C D 1440, iOS 390, And 360. Long brief: "brief: … uncl…(168)" → Show all (fb 0–1) → the whole 474 chars, then cwd and role; "Show less". No "(Show all)", no JSON. The helper card (cwd, role) and the project card have no Show all. Shots `h-setup-hsetup1-showall-*` |
| 10 [minor] a row with no summary takes three lines | `row.ts rowLines`: on every two-line list a row with no answer puts its run line second (as phones did), with no third line; a phone's nested run row keeps its run second, as before (caught by the delegate review). `TaskRow.tsx` uses it | `shell-list row-two-lines.test.ts a_row_with_no_answer_is_two_lines_the_run_second_audit_g_tasks_10` (pure, new) | C D 1440, W D 1280: "TOPIC[Setup card] Add a reviewer…" is "Claude Code · Report" on line 2. All 45 rows are 55 px; none has an empty second line or a third line. Shot `h-row-nosummary-*` |

## Audit E #2 and audit F #1, carried over

| # | Fix | Test | Proof (live) |
|---|---|---|---|
| E 2 phone sheet strip at 360 with a pinned tab | **No code change.** The strip already scrolls inside the screen (QA r7 #5): the scroller is `panel.tabs` (`overflow-x: auto`). Its tab list keeps its full width and never scrolls itself, and that is the element the audit measured (`scrollWidth == clientWidth`). The end it can still scroll to fades | `ui phone-sheet-strip-scroll.test.ts the_{ios,android}_side_sheets_tab_row_scrolls_inside_the_screen_audit_e_2` (a regression lock on the looks' rules) | C iOS 360 D: scroller x 16–344, at rest `data-scroll-end`. A CDP touch swipe → scrollLeft 33, Thread at 290–342, `data-scroll-start`. And 360 D: scroller 6–344; swipe → 48, Thread 289–344. Shots `h-e2-swiped-ios-360-dark`, `h-e2-sheet-android-360-dark` |
| F 1 deep link `?side=shell:…` focuses the reply box | `ShellPane.tsx`: a pane opened before the user has pressed anything (`navigator.userActivation.hasBeenActive === false`: a deep link, a reload) takes the keys even from the reply box, which had focused itself. A field the user moved to keeps them, as before | `shell-task shell-focus.test.tsx a_deep_link_to_the_shell_takes_the_keys_from_the_reply_box_audit_g_tasks_f1` (**seen failing**); `a_reply_box_the_user_moved_to_keeps_the_keys_when_the_shell_starts` | C and W D 1280 and 1440: open `?side=shell:local:<cli1>` → focus "Terminal input"; `echo f1` goes out as `terminal.input`; the reply box stays empty. Shot `h-shell-deeplink-*` |

## Gates

- `npm run typecheck`: 4 programs, 0 errors.
- `npm run check`: 0 errors, 557 warnings. The warnings on touched files are the existing size warnings on
  HomeView, TaskView and Task.tsx.
- vitest, every touched or added file: `factory-pick`, `phone-bar`, `row-two-lines`, `setup-card`, `shell-focus`,
  `ask-text-wrap`, `lazy-sheet-title`, `phone-answer-target`, `phone-sheet-strip-scroll`, plus `lazy-context-menu` and
  `menu-sheet` (next to the Menu change). 11 files, 41 tests, all pass.
- Rust (`CARGO_TARGET_DIR=/tmp/wisp-target-fixer`):
  - `cargo fmt --all --check`: clean;
  - `cargo clippy -p wisp-runtime-core --all-targets -- -D warnings`: clean;
  - `cargo test -p wisp-runtime-core --lib setup`: 9 passed.
- `node tools/budgets.ts`: exit 0, under every budget; no budget was raised.
  - first-screen JS 172.1 KB br desktop (budget 173.0), 165.9 KB br phone (167.0);
  - first-screen CSS 28.8 KB br desktop (30.0), 39.9 KB br phone (40.5).
- Console: clean in every run, in Chrome and WebKit.
- A delegate review of the diff found one regression, which is fixed and covered: `rowLines` had put a phone's nested
  run row with no answer back to three lines.
