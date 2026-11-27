# ADR-0140: every loader is the Liquid

- Status: accepted
- Date: 2026-11-27
- Source: owner, 2026-11-27: "be creative and find a better loader than the grid", then, from two rounds of mocks,
  "liquid looks so good i love it. Find other loading in the app and use the same pattern."
- Changes: a new kit component `Liquid` (`packages/ui/src/content/Liquid.tsx`, `css/liquid.css`). `Orb` and `Spinner`
  keep their names and props but draw it. Departs from ADR-0016 (v2 looks like v1) for loaders only.

## Context

v1 drew live work as a 3×3 dot grid with a ripple (`.orb.live`). Buttons and pending controls had a border ring that
spun, and other places each had their own busy mark: three pulsing
dots for a /btw answer, a pulsing dot for the current step and a running workflow step, a pulsing clock on an
upload, and a spinning replay icon on Re-check. Seven marks for one meaning. The owner turned down the grid and a first
round of alternatives (a flame, a comet, a snake in the grid, bars, braille, orbiting dots, ripples). In a second
round they picked "Liquid".

## Decision

1. **One mark.** `Liquid` draws three drops in `currentColor` on a 24 grid. They orbit one centre on different periods
   (2.4 s, 2.9 s and 3.4 s), so the shape doesn't visibly repeat. One SVG filter (a Gaussian blur, then a sharp alpha
   threshold, as metaballs) melts them into one surface where they meet. The filter is put in the document once, as
   the icon sprite is (`#k-liquid`), so the markup is the same everywhere and goldens stay stable.
2. **States.** Working: the accent colour. Waiting on you: amber (`--warn`), slower (4.2 s). Failed or idle (`still`)
   and `prefers-reduced-motion`: the drops rest where they start, as one still blob.
3. **Everywhere.** `Orb` (the working panel, inline and boxed, and the Watch pane), `Spinner` (a Button's or
   IconButton's `loading`, `ActIcon`'s pending act, settings sign-in), the current `Steps` item, a
   running `RunBoard` mark, an uploading `FileChip`, the /btw answer on its way and launcher setup's Re-check all
   draw it at the size of what they replace.
4. **What stays.** The sandbox's `BuildBox` and `BuildBoxMark` (a sandbox starting, its task row; #1038) have their own, recent design: the owner kept it
   (2026-11-27). Skeletons (a placeholder in the real thing's shape) are layout, not loaders. Status dots in pills
   and list rows (`pill-dot`, `rn-dot`, the crew tile's dot) say a state at 5–7 px, where the mark can't read. Pull
   to refresh keeps the platform's own indicator because it is the gesture's feedback (design/mobile.md). The
   `run-card` sweep line is the card's border, not its mark.
5. **Guarded.** `packages/ui/test/liquid.test.tsx` checks that every kit loader draws the Liquid, that the filter is
   in the document once, the states, and that no kit CSS spins or pulses a loader of its own. Decision-ledger
   proposal P-cc6fb3dd carries the rule to plugins and the shell.

## Consequences

- `qa/parity.mjs` will show the orb and the spinner as differences from `design/v1-reference/`. They
  are intended, and their references are re-captured on the next parity run.
- The filter repaints a mark's own box (at most 20 px) each frame. Only transforms animate, and the area is tiny next
  to the transcript, so the streaming budget (0 long tasks) is not affected.
