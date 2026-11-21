# ADR-0068: A task's earlier turns are a ledger led by outcomes, with day cards, notice folds and a scrubber

- Status: accepted
- Date: 2026-11-15
- Source: the owner's pick of the three directions in `docs/design/history-redesign.md` (mock `history-redesign.html`,
  research `docs/research/long-task-history.md`): **A only**, Ledger + scrubber, on every width, with the open turn
  expanding in place on phones too (C's stage page and B's chapters are not built).
- Supersedes ADR-0016 **for this one screen part** (the list of a task's earlier turns): it no longer reproduces v1's
  folded turns and *Show N earlier turns*. Everything else on the task page (head, details, the latest turn, the
  reply box, the scroll rules in `design/ux.md`) still follows ADR-0016. With ADR-0011 (budgets), ADR-0017 (one
  frame), ADR-0051 (the turn window), ADR-0067 (lazy code and strings).

## Context

- A long task (the mock: 432 turns over 6 days, ~20 % of them wisp's own notices) showed its last 3 turns and
  *Show 426 earlier turns*. Each folded row was the first line of the user's prompt: what was asked, not what came
  of it. wisp's notices ("Subagents you delegated to earlier have finished…") looked exactly like the owner's own
  prompts. Opening a row poured the whole turn inline and lost the reader's place.
- The window (ADR-0051) sends turns older than the latest 8 as stubs: hero, status, `by`, `changedFiles`, but not
  the paths they changed, so the history couldn't be searched by file.

## Decision

1. **Rows lead with the outcome.** Every turn before the latest is one line: a status glyph (done, failed, answer,
   running in the kit's state colours), the answer's hero title (the prompt's first line when it has none) and the
   time; the user's prompt is the second line (`You: …`). The model is `packages/ui/src/app/ledger-model.ts`
   (`@wisp/ui/pure`); a turn row is read by `packages/shell/src/frame/ledger-turns.ts` (`@wisp/shell/pure`):
   failed = status `error`, `isError`, a refusal or a `blocked` hero; answer = an `info` hero, or no changed files.
2. **wisp's notices** are turns whose `by` is set and isn't the user's (`user`, `you`, `step`). They get the spark
   glyph, a faint colour and no second line (`wisp · <kind> → <hero>`; a `wisp` notice's kind is its prompt's first
   clause, others a word per sender: `later` → *Scheduled follow-up*, `crew`, `lead`, `agent`, `channel`,
   `compact`). **Three or more in a row fold into one line** (`4 wisp notices · CI finished, Resumed`); a press
   unfolds that run; a run holding the open turn never folds.
3. **Days.** Today and yesterday are open, under a day head with *Fold*. Earlier days are a **day card**: its name,
   turn count, time span, `N done, N failed, N files` and an outcome strip (one tick per turn, red for failed). A
   press opens the day; the open turn's day is always open. *Show N earlier turns* is gone.
4. **Search and filters** (a task with more than 10 turns): one box over heroes, prompts, notice kinds and file
   paths (every word, anywhere, case-blind), and chips *All · Yours · Failed · Shipped · wisp notices* with their
   counts. *Shipped* is a filter, not a glyph: a turn with a `commits` block with commits or a `graph` block of an
   open or merged PR. Searching or filtering turns the ledger into a flat list of matches under a head per day.
5. **The scrubber** (a task with more than 30 turns): one tick per earlier turn down the right edge (red failed,
   short faint for a notice, accent for the open one), sticky as the page scrolls and as tall as what shows of it.
   Hovering previews the turn (day, time, `turn n of N`, glyph and hero); a press, or a drag's end, opens that turn
   (clearing a search that hides it) and brings it into view. Pointer events (mouse, touch, pen alike); ArrowUp/Down
   step when it has focus. 56 px wide (`--size-scrub`), 22 px under 520 px of ledger width (`--size-scrub-phone`).
   Nothing scrolls smoothly: jumps are instant, so it's `prefers-reduced-motion` safe.
6. **One turn open at a time, in place, on every width.** Its head row stays where it was on screen (the page is
   scrolled by however much a turn closing above it moved it); under it the turn whole (the full prompt, the hero
   with its stats and files, the answer, its actions), then *‹ Earlier · Later ›*. Opening a stub asks for that
   turn whole (ADR-0051's `/turn/<id>`). The latest turn (the live one) is drawn whole after the ledger as before:
   the reply box, the live stream and `design/ux.md`'s scroll rules are unchanged. A reader who left the end keeps
   the turns they were reading whole as new ones land (F-page-44, `foldedFrom` with `keep = 1`).
7. **Keys, while focus is in the ledger** (a row, the search box, the scrubber, the open turn): `j`/`k` step to the
   next or previous turn (within the matches while searching), `/` focuses the search, `[`/`]` jump to the previous
   or next failure, `Esc` clears the search, then closes the open turn, then goes on to the page's Back. Outside the
   ledger those keys keep their v1 meaning (`j`/`k` the next task, `/` the reply box, `[` the list, `]` the panel):
   the ledger handles its keys on its own element and calls `preventDefault`, which the shell's dispatch respects.
8. **A task with more than 200 earlier turns** draws its ledger as a `VirtualList` (its own scroller, as tall as its
   lines up to `--size-sheet-max`); a jump to a line not drawn scrolls there by its index first. It's decided by the
   task's size, not by the lines on show, so opening a day or typing a search never swaps the list under the reader.
9. **The window's stubs keep `files`** (the paths, once each, at most `STUB_FILES` = 100) **and `shipped`**
   (`plugins/runtime/core/src/turn_window.rs`): additive body fields; a whole turn's blocks and changes say the same
   (`ledger-turns.ts` reads either). No new wire message, topic or command.
10. **Kit parts** (`packages/ui/src/app/Ledger.tsx`, `css/ledger.css`): `task.ledger` and its `.row`, `.open`,
    `.fold`, `.day`, `.dayhead`, `.fold-day`, `.tools`, `.search`, `.filter`, `.scrubber`, `.preview`, `.earlier`,
    `.later`, `.virtual`. Its sheet and code load with the task page (never the first screen: ADR-0011, ADR-0067);
    its strings are `shell.ledger.*` in `i18n/en.lazy.json`.

## Edge cases (each with its test)

| Case | Behaviour | Test |
| --- | --- | --- |
| Turns over several days | today and yesterday open, earlier days a card with count, span, done/failed/files, strip | `packages/ui/test/ledger-model.test.ts` `today_and_yesterday_open_earlier_days_fold_into_a_card_with_counts_span_files_and_a_strip` |
| A day opened or folded by hand; the open turn on a folded day | as chosen; the open turn's day shows | same file, `a_day_opens_or_folds_by_hand_and_the_open_turns_day_always_shows` |
| Just after midnight; a DST day | a turn from 23:50 is yesterday's; day kinds follow local midnight | same file, `day_kinds_follow_local_midnight` |
| A turn whose time is out of order (an import, a skewed clock) | joins its own day; one group, unique keys | same file, `a_turn_out_of_time_order_joins_its_own_day_which_stays_one_group_with_one_key` |
| A day with no turns | zero stats | same file, `day_stats_of_no_turns_are_zero` |
| 4 notices in a row | one fold line, kinds once each, keyed by its first turn | same file, `three_or_more_notices_in_a_row_fold_into_one_line_with_their_kinds_once_each` |
| 2 notices; an unfolded run; a run holding the open turn; a run at the end | rows; rows; rows; folds | same file, `two_notices_stay_rows_an_unfolded_run_and_a_run_holding_the_open_turn_show_every_row` |
| Search by path, by two words, by prompt; no match | found; every word anywhere; found; nothing | same file, `search_reads_heroes_prompts_and_file_paths_every_word_anywhere_case_blind` |
| Each filter and its count | Yours = not notices, Failed, Shipped, wisp notices | same file, `filters_yours_failed_shipped_and_notices_and_their_counts` |
| Searching or filtering | a flat list under a head per day, no cards or folds; no match is empty | same file, `searching_gives_a_flat_list_of_matches_under_a_heading_per_day_without_cards_or_folds` |
| A turn without a hero | titled by its prompt's first line | same file, `a_row_without_a_hero_is_titled_by_its_prompts_first_line` |
| Scrubber ticks | one per turn, each in the middle of its slot, tones failed / notice / on | same file, `one_tick_per_turn_spread_evenly_red_for_failed_faint_for_notices_accent_for_the_open_one` |
| A point above, below or on the scrubber; no turns; no height | clamped to the ends; none | same file, `a_point_on_the_scrubber_names_the_turn_under_it_clamped_to_both_ends` |
| j/k at either end, with none open, with a gone open turn | undefined past the ends; k the latest, j the first | same file, `j_and_k_step_from_the_open_turn_and_with_none_open_k_takes_the_latest_j_the_first` |
| [ / ] | the previous or next failure; with none open the latest or first | same file, `brackets_jump_to_the_previous_or_next_failure` |
| A key with ⌘/Ctrl/Alt | not the ledger's | same file, `the_ledger_keys_are_j_k_slash_brackets_and_escape_without_modifiers` |
| A turn row: hero, number, files once each | as stated | `packages/shell/test/ledger-turns.test.ts` `a_row_is_led_by_its_hero_numbered_from_one_with_its_prompt_whole` |
| `by` user/you/step vs wisp/later/crew/…; an empty wisp notice | the user's; notices; its sender's word | same file, `wisp_notices_are_told_apart_from_the_users_prompts_and_a_workflow_step_is_the_users` |
| A long notice | its first clause, cut at 48 | same file, `a_notices_kind_is_its_first_clause_cut_at_48` |
| error, isError, refusal, blocked/info/done hero, running, held | failed ×4, answer, done, running ×2 | same file, `the_outcome_is_failed_on_an_error_a_refusal_or_a_blocked_hero_done_when_it_changed_files_else_an_answer` |
| The same turn whole and as a stub | reads the same (paths, Shipped) | same file, `a_stub_reads_the_same_as_its_whole_turn_paths_and_shipped_from_its_own_fields` |
| commits with/without commits; open/merged/closed PR | shipped, not, shipped, not | same file, `shipped_is_a_commits_block_with_commits_or_an_open_or_merged_pr` |
| A stub's paths (runtime's or a v1 body's), duplicates, empty, more than 100; shipped | once each, in order, first 100; the edits still go | `plugins/runtime/core/src/turn_window.rs` `a_stub_keeps_the_paths_it_touched_and_whether_it_shipped_for_the_ledger_adr_0068` |
| Opening a row while a turn above it is open | the pressed row stays put on screen | not unit-tested (layout); checked in Chrome at 1280 and 390 px (target/r235/) |
| The ledger's keys while typing in the reply box or an answer's field, or with focus in the latest turn | not the ledger's (they type; the latest turn keeps the page's keys) | not unit-tested (DOM focus); `ledgerKey` covers which keys, `Ledger.tsx` skips inputs and `ledger-tail` |
| A right-click, or a drag that began elsewhere, ending on the scrubber | opens nothing | not unit-tested (pointer events); `Ledger.tsx` `pressed` |

## Consequences

- The task page no longer matches v1's pixels for its earlier turns; `qa/parity.mjs` must list this part as an
  allowed difference (`design/v1-reference/allowed.md`) when the batched pass runs.
- Stub rows grow by the paths a turn touched (capped at 100), a few hundred bytes per turn at most.
- The ledger's keys only work with focus in it; a reader who never clicks into the history gets v1's keys. A later
  change could make them page-wide once v1's j/k (next task) has another home.
- The scrubber covers the earlier turns, not the latest one (that one is always open below it).
