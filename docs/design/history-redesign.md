# Task history: three directions

Mock: [history-redesign.html](history-redesign.html). Open it in a browser. It has a switch for each
direction, desktop / phone / both, and light / dark. The data is one task with 432 turns over 6 days;
about 20% of the turns are wisp's own notices. On the desktop frame: `j`/`k` step, `/` searches,
`[`/`]` jump to the previous or next failure, `Esc` closes. Screenshots are in `history-redesign/`.

**What's wrong today.** A row is the first line of *your* message, which says what you asked and
not what came of it. 426 turns hide behind one link. wisp's notices ("Subagents you delegated to
earlier have finished…") look exactly like your own prompts. Opening a row pours the whole turn
inline, so you lose your place.

**What all three share.** A turn is labelled by its outcome (the answer's hero: *Reconnect test fixed*),
with a status glyph (done / failed / answer) in the kit's state colours. Your message comes second.
wisp notices get the wisp glyph, a faint colour and no second line, and a run of them folds into one
line. Every direction has search over heroes, prompts and file paths, plus filters: *All · Yours ·
Failed · Shipped · wisp notices*. "Show N earlier turns" is gone: nothing loads empty and nothing
is hidden behind a count.

## A · Ledger + scrubber

*Every turn is one outcome-led line, and a scrubber down the right edge holds the whole task. You
skim it like a commit log and drag to any point.*

- **Older turns compress:** today and yesterday are open. Earlier days fold into a day card: turn
  count, time span, phases worked on, "39 done, 6 failed, 3 files", and an outcome strip (one tick
  per turn, red for failed).
- **Jumping:** a scrubber with a tick for each of the 432 turns (red = failed, short faint = wisp
  notice, accent = open). Hovering shows a preview (date, time, turn number, hero) and a click opens
  that turn. Search and filters turn the list into a flat list of matches grouped by day. Keys:
  `j`/`k`, `/`, `[`/`]`.
- **An opened turn** expands in place. Its head row stays put: what you asked in full, then the hero
  with tests, files and time, then the answer, the files and *‹ Earlier · Later ›*. One turn is open at a time.
- **System messages:** one faint line reading "wisp · CI finished → PR #795 opened". Three or more
  in a row fold to "4 wisp notices · CI finished, Resumed".
- **Phone:** the same list, with a 22px scrubber on the right edge (like Photos' fast scroll), and the
  filters scroll sideways.
- **Kit cost (low):** an outcome row (`.row` with hero and sub-line), a day summary card, the
  scrubber (new: about 120 lines, pointer events, `prefers-reduced-motion` safe), a notice fold line,
  and the existing search and filter chips.

## B · Chapters

*The task reads as a story in chapters (one per stretch of work: "Install gate sockets", "Reconnect,
for good"). A closed chapter is a summary; an open one is a run of cards led by their heroes.*

- **Older turns compress:** into chapter cards. Each lists its top three results, "N turns failed
  on the way", tests before → after, files touched and the number of notices. 432 turns become 7 cards.
- **Jumping:** a table of contents at the side (desktop) with an outcome bar per chapter, or a
  chapter chip bar (phone). Search opens only the chapters that match and shows "3 of 64 turns".
- **An opened turn:** its card lifts (`--shadow-2`) and shows the answer, the files and actions. Your
  message is the card's footnote ("You: …"), in full once the card is open.
- **System messages:** they sit in a quiet lane on the time rail, outside the cards ("○ 14:45 ·
  Scheduled follow-up · PR #795 opened"). A run of them folds.
- **Phone:** no rail and no TOC. Cards are full width with the time on top, and the chapter chips
  sit under the search.
- **Kit cost (high):** chapters need a grouping signal. That means a phase boundary from the server
  (topic shift, a PR opened, a day, a goal), named by a cheap summary, plus a fallback per day when
  there's no name. It also needs a turn card, a chapter summary card, a TOC, the time rail and lane,
  and a chapter chip bar. The naming is product work, not look and feel.

## C · Outline + stage

*One turn at a time takes the stage and reads like a page. Everything else is a dense numbered
outline beside it, or under your thumb on a phone.*

- **Older turns compress:** to one 28px outline line each (number, glyph, hero, time), grouped by
  day. All 432 can sit in a virtual list without a fold.
- **Jumping:** the outline has search and filters. Keys: `j`/`k` step, `[`/`]` jump between
  failures, `/` searches. The stage has ↑/↓ buttons and cards for the turns before and after. On a
  phone, a bar above the composer ("432 / 432 · Reconnect test fixed · ↑ ↓") opens the outline as a sheet.
- **An opened turn:** your message as a quiet block, the hero as the page heading (26px, the
  screen's one display size), stats, the answer, files, actions, and the turns before and after.
  Nothing else on screen moves.
- **System messages:** a faint line in the outline ("wisp · Scheduled follow-up"). On the stage they
  replace "You asked" with a dashed notice box: "wisp sent this · Scheduled follow-up".
- **Phone:** the stage is the page. The outline is a bottom sheet (78% high, drag handle, scrim),
  stepping uses 44px buttons, and there's no keyboard hint on touch.
- **Kit cost (medium):** a split pane inside the task panel, a virtual outline list, the stage
  page, a bottom sheet (Material 3 standard), and the near-turn cards. It also changes the
  task panel's shape: 340px of outline beside the crew rail and task list is tight below 1280px.
  Below about 1100px it would have to fall back to the sheet.

## Recommendation

**Build A, and take C's stage for an opened turn on phones.**

- A answers the owner's two complaints without changing how a task page works. Older turns become
  readable (outcomes, not prompts) and reachable (day cards plus the scrubber, so there's no "Show
  426"). wisp's notices stop looking like the owner's messages. It keeps the composer, the stream
  of the live turn and the scroll rules in `design/ux.md` (you own the scroll) as they are.
- B is the most beautiful at 400+ turns. It depends on naming chapters well, though, and a wrong
  name is worse than none. Ship A's day cards first; if phase boundaries appear later, a day card
  can carry a chapter name without a redesign.
- C is the best for reading one old turn. It's the worst for the live turn, which wants the stream
  and the composer together, and on desktop it takes a third column. On a phone, though, opening an
  old row as a stage page with ↑/↓ beats expanding 40 lines inline in a 390px list. So: A's list
  everywhere, A's inline expand at desktop width, C's stage page on phones (Back returns to the
  same row).

Open questions for the owner: whether the scrubber shows on every task or only above about 30 turns
(I'd say above 30), and whether *Shipped* should be a filter or a glyph.
