// UX 2026-11-18 desktop #13, QA round 6 desktop #1: the Shell drew boxes (□) where a prompt has powerline / Nerd Font
// glyphs. Listing the Nerd Font families helps only where one is installed, so wisp brings a symbols-only font of its
// own (Nerd Fonts' Symbols Nerd Font Mono, the glyph sets with a clear licence), after the user's monospace and any
// Nerd Font they have. It is two files the browser fetches only when a row shows a glyph in its range (unicode-range on
// FontFaces the Terminal adds when it mounts), so no page that never shows a terminal, the first page included, loads it.
import { expect, test } from 'vitest';
import CSS from '../src/css/terminal.css?raw';
import CORE from '../src/terminal/fonts/symbols-core.woff2?inline';
import MDI from '../src/terminal/fonts/symbols-mdi.woff2?inline';
import FONT_LICENCES from '../src/terminal/fonts/LICENSE.md?raw';
import { SYMBOLS, SYMBOL_FACES, addSymbolFaces } from '../src/terminal/symbols.ts';

test('the_shells_font_falls_back_to_installed_nerd_fonts_then_wisps_own_symbols', () => {
  const rule = CSS.match(/\[data-kit='term-rows'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  const at = (s: string): number => rule.indexOf(s);
  expect(at('var(--mono)')).toBeGreaterThanOrEqual(0);
  expect(at("'MesloLGS NF'")).toBeGreaterThan(at('var(--mono)'));
  expect(at(`'${SYMBOLS}'`)).toBeGreaterThan(at("'MesloLGS NF'"));
  expect(rule).not.toMatch(/url\(/); // no font file named in the kit's CSS: nothing for a page without a terminal
});

test('the_symbol_faces_are_added_once_each_for_its_own_glyph_ranges', () => {
  const added: { family: string; source: string; range: string | undefined }[] = [];
  class Face {
    readonly family: string;
    readonly source: string;
    readonly d: { unicodeRange?: string } | undefined;
    constructor(family: string, source: string, d?: { unicodeRange?: string }) {
      this.family = family;
      this.source = source;
      this.d = d;
    }
  }
  const fonts = { add: (f: Face) => void added.push({ family: f.family, source: f.source, range: f.d?.unicodeRange }) };
  addSymbolFaces(fonts, Face as never);
  addSymbolFaces(fonts, Face as never);
  expect(added.length).toBe(SYMBOL_FACES.length);
  expect(added.every((f) => f.family === SYMBOLS && /^url\(".+\.woff2"\) format\('woff2'\)$/.test(f.source))).toBe(true);
  // powerline's arrows and git branch, Octicons' branch, and Material Design's apple are each in one face's range
  const ranges = added.map((f) => f.range ?? '').join(',');
  const covers = (cp: number): boolean =>
    ranges.split(',').some((r) => {
      const [a = '', b = a] = r.trim().replace('U+', '').split('-');
      return cp >= parseInt(a, 16) && cp <= parseInt(b, 16);
    });
  for (const cp of [0xe0b0, 0xe0a0, 0xf418, 0xf0035, 0xe702]) expect(covers(cp), cp.toString(16)).toBe(true);
  expect(covers(0x41), 'never plain text').toBe(false);
  expect(covers(0xf302), 'Font Logos: no clear licence, left out').toBe(false);
  expect(covers(0xe000), 'Pomicons: OFL with a Reserved Font Name, left out').toBe(false);
  expect(added[0]?.range).toContain('U+E0A0-E0D7');
  expect(added[1]?.range).toContain('U+F0001-F1AF0');
});

test('the_font_files_are_small_and_their_licences_are_noted', () => {
  // a data: URL's base64 is 4 characters for every 3 bytes
  for (const f of [CORE, MDI]) expect((f.length - f.indexOf(',') - 1) * 0.75).toBeLessThan(600_000);
  // the repo's notice names it too: tools/test/notices.test.ts
  expect(FONT_LICENCES).toMatch(/Ryan L McIntyre/);
});
