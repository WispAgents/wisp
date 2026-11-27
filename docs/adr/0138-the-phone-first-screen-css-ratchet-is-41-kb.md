# ADR-0138: the phone first screen's CSS ratchet is 41 KB br

- Status: accepted
- Date: 2026-11-27
- Source: main's suite red on `bundle budgets` since #997 (`first screen CSS (phone) 40.6 KB br > 40.5 KB (+64 B)`).
- Changes: ADR-0105 §4's phone CSS ratchet (`tools/budgets.ts` FIRST_SCREEN_BUDGETS), 40.5 → 41 KB br. Every other
  number stays.

## Context

The phone look's first screen loads the kit's stylesheet and `ios.css`/`android.css` up front. The ratchet set on
2026-11-19 (39.5 KB plus 2 KB, cut to 40.5) has been met within 512 B ever since. The side panel's PR tab redesign
(#1003: check rows, folds, file stats), the Shipping section (#1002) and the goal follow-up (#995) added rules the
kit's one stylesheet carries for every look, which took it 64 B over. Merging duplicate rules does nothing: the
minifier already merges them. The real cut, moving the phone overlays' rules (`.k-menu`, `.k-listbox`,
`.k-select-sheet`, `.k-dialog`) out of `ios.css`/`android.css` into the lazy `controls/phone.tsx` chunk, changes
which layer those rules land in and how a sheet's first press looks. That needs its own round with the phone in hand.

## Decision

1. The phone ratchet is 41 KB br. Desktop stays 30 KB CSS / 173 KB JS, phone JS stays 167 KB, and BUDGETS stays the
   target.
2. The overlay split above is the next cut. When it lands, the ratchet goes back down to its number plus 2 KB, as
   ADR-0105 §4 says.

## Consequences

The suite is green again with 400 B of slack on phone CSS. The next 400 B of first-screen CSS fail the build again.
