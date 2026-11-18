# ADR-0101: The terminal brings its own symbols font, loaded per glyph

- Status: accepted
- Date: 2026-11-18
- Source: QA round 6 desktop #1 (the Shell drew boxes for a prompt's symbols), after UX 2026-11-18 desktop #13 (the
  terminal lists the Nerd Font families: that helps only where one is installed).

## Context

Prompts made for powerline, starship or powerlevel10k use private-use glyphs (U+E0A0 branch, U+E0B0 arrows, Octicons,
Devicons, Material Design marks) that only a Nerd Font has. Most machines have none, and phones never do, so the
Shell showed empty boxes where the user's own terminal shows their prompt. wisp aims to be fast (ADR-0011): the fix
must not add a byte to the first page or to a page that shows no terminal.

## Decision

- **A new third-party asset** (a dependency by AGENTS.md): Nerd Fonts v3's Symbols Nerd Font Mono, cut with fonttools
  to the glyph sets with a clear licence and converted to WOFF2, in two files under `packages/ui/src/terminal/fonts/`:
  `symbols-core.woff2` (Powerline, Powerline Extra, Font Awesome and its Extension, Seti-UI, Devicons, Codicons,
  Octicons, IEC; 556 KB) and `symbols-mdi.woff2` (Material Design Icons; 504 KB). Licences in `fonts/LICENSE.md` and
  `THIRD-PARTY-NOTICES.md` (Font Awesome's glyphs are SIL OFL 1.1 in font files, its icons CC BY 4.0; no file carries a
  Reserved Font Name); Font Logos (no licence upstream), Pomicons and Weather Icons (rarely in prompts) are left out.
- **Loaded per glyph, by the terminal only:** the kit's Terminal adds two FontFaces (`wisp Symbols`) when it mounts,
  each with its `unicode-range` (`terminal/symbols.ts`). The browser fetches a file only when a row shows a glyph in its
  range. No CSS names a font URL, so the first page and every page without a terminal are unchanged.
- **Last in the stack:** the terminal's font is the user's monospace (`--mono`), then the Nerd Font families they may
  have installed, then `wisp Symbols`, then `monospace`. An installed Nerd Font wins; ours fills only what's missing.

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| The font order: mono, installed Nerd Fonts, then wisp's; no URL in the kit's CSS | `packages/ui/test/term-font.test.ts` `the_shells_font_falls_back_to_installed_nerd_fonts_then_wisps_own_symbols` |
| The faces are added once, each for its own ranges; plain text and Font Logos are in none | `…` `the_symbol_faces_are_added_once_each_for_its_own_glyph_ranges` |
| The files stay small and their licences are noted | `…` `the_font_files_are_small_and_their_licences_are_noted` |
| The glyphs draw in Chrome and WebKit, and the first page doesn't grow | checked in the browser and with `tools/budgets.ts` (QA round 6 report) |

## Consequences

- 1 MB of fonts in the repo and in the app bundle, fetched by a client only when its terminal shows such a glyph.
- Updating the font is re-running the subset (the ranges in `symbols.ts`) on a newer Nerd Fonts release.
