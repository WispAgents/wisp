# Third-party notices

Assets wisp ships that were made by others, with their licences. Code dependencies are listed by their package
managers (`Cargo.lock` with `cargo deny`'s licence policy in `deny.toml`, `package-lock.json`); this file is for what
is copied into the repo.

## Symbols Nerd Font Mono (the terminal's symbols font)

- Files: `packages/ui/src/terminal/fonts/symbols-core.woff2`, `symbols-mdi.woff2` (ADR-0101).
- What: a subset of Nerd Fonts v3's Symbols Nerd Font Mono (https://github.com/ryanoasis/nerd-fonts), the glyph sets
  with a clear licence, converted to WOFF2. Loaded only by the terminal, only for a glyph in its ranges.
- Licences: Nerd Fonts MIT (© 2014 Ryan L McIntyre); glyphs MIT (Powerline, Powerline Extra, Font Awesome Extension,
  Seti-UI, Devicons, Octicons, IEC Power Symbols), Font Awesome Free © Fonticons, Inc. (SIL OFL 1.1 in font files,
  "Font Awesome" a Reserved Font Name that no file here carries; CC BY 4.0 for the icons), CC BY 4.0 (Codicons ©
  Microsoft Corporation) and Apache 2.0 (Material Design Icons © Pictogrammers). Left out: Font Logos (no licence
  upstream), and Pomicons and Weather Icons (OFL; prompts rarely use them).
- Full texts: `packages/ui/src/terminal/fonts/LICENSE.md`.
