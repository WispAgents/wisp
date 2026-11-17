# ADR-0095: A turn's trail has an inline slot after its token count

- Status: accepted
- Date: 2026-11-17
- Source: docs QA round 1 #19 (the phone task page: each turn's foot on three lines). Adds an extension point (AGENTS.md:
  a new extension point needs an ADR); follows ADR-0003 (everything is a plugin) and ADR-0012 (nothing wraps that
  shouldn't).

## Context

v1 drew a turn's receipt inside its trail bar, after the token count: *2292 tokens · 90% cache · 3% of weekly*
(turns.js receiptHtml). In v2 the receipt is shell-task's (`receiptOf`: the cache share needs the turn's stats, the
plan share wisp.budget's ext slot), and the only place a plugin could draw under a turn was `shell.turn:foot`, a stack
under the trail. So the receipt took a line of its own: on a 390 px phone the trail became three lines (the counts, the
tools, then "90% cache"), one more than v1, and on desktop two where v1 had one.

## Decision

A new slot, `shell.turn:trail` (part `turn.trail`), drawn by the task view inside the trail's token-count item, after
its words, `layout="contents"`, with the turn's key as its id (as `shell.turn:foot`). It is for a few quiet words
that belong on the count's line. shell-task's `shell-task.receipt` (`TrailReceipt`) attaches there and draws
" · 90% cache · 3% of weekly", its title every window and the whole count; the receipt leaves `shell.turn:foot`.

The kit's `Task` takes it as `receiptMore` (additive): it sits in the same element as the token count, so the phone's
trail grid (app-stack.css, four columns) keeps one cell for it and the line never grows a fifth column.

## Consequences

- A phone's trail is two lines again (counts with the receipt, then the tools), as v1's; the desktop's one.
- A plugin may add words to the count's line; anything taller still goes in `shell.turn:foot`.
- No receipt without a token count (the slot lives in the count's element): the receipt's cache share needs the count.

## Places it reaches (ADR-0040)

The browser and the desktop app (the task view). The phone apps draw the same task view. `wisp tui` and the CLI show
the receipt from the same turn data; nothing to change there.

## Edge cases and their tests

- The receipt's words after the count, and nothing when there are none: `plugins/shell-task/ui/test/trail-receipt.test.ts`
  `the_receipt_follows_the_token_count_on_the_trails_line_qa_r1_19`.
- What the receipt says (cache share, longest window, Past your plan, none without a cache read): `cards.test.ts`
  `each_card_says_why_a_turn_stopped_was_held_what_changed_and_what_it_cost` (unchanged).
