// The terminal's own symbols font (QA round 6 desktop #1, UX 2026-11-18 #13; ADR-0101): Nerd Fonts' Symbols Nerd Font
// Mono, cut to the glyph sets with a clear licence (fonts/LICENSE.md), in two files. They're added as FontFaces when a
// Terminal mounts, each for its own glyph ranges, so the browser fetches a file only when a row shows a glyph in it:
// a page without a terminal (the first page included) never loads either. After the user's monospace and any Nerd Font
// they have installed (terminal.css), so a prompt made for those draws here as it does in their own terminal.

/** the family name terminal.css lists after the installed Nerd Fonts */
export const SYMBOLS = 'wisp Symbols';

/** each file and the code points it holds (pyftsubset's cut of SymbolsNerdFontMono-Regular, Nerd Fonts v3) */
export const SYMBOL_FACES: readonly { readonly url: string; readonly range: string }[] = [
  {
    // Powerline (and Extra), Font Awesome Extension, Seti-UI, Devicons, Codicons, Font Awesome, Octicons, IEC
    url: new URL('./fonts/symbols-core.woff2', import.meta.url).href,
    range: 'U+E0A0-E0D7, U+E200-E2A9, U+E5FA-E6B7, U+E700-E8EF, U+EA60-EC1E, U+ED00-F2FF, U+F400-F533, U+2665, U+26A1, U+23FB-23FE, U+2B58',
  },
  // Material Design Icons (starship's and many themes' OS and tool marks)
  { url: new URL('./fonts/symbols-mdi.woff2', import.meta.url).href, range: 'U+F0001-F1AF0' },
];

type Fonts = { add(f: unknown): unknown };
type FaceCtor = new (family: string, source: string, d?: { unicodeRange?: string; display?: string }) => unknown;
const done = new WeakSet<object>();

/** Add the faces to a document's fonts once (unloaded: a glyph in range loads its file). */
export function addSymbolFaces(fonts: Fonts | undefined, Face: FaceCtor | undefined): void {
  if (!fonts || !Face || done.has(fonts)) return;
  done.add(fonts);
  for (const f of SYMBOL_FACES) fonts.add(new Face(SYMBOLS, `url("${f.url}") format('woff2')`, { unicodeRange: f.range, display: 'swap' }));
}
