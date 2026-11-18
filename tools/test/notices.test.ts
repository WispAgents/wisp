// THIRD-PARTY-NOTICES.md names what wisp ships that others made, with their licences (ADR-0101: the terminal's symbols
// font). Each asset it names exists, and the font's entry says every licence its glyph sets carry.

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import test from 'node:test';

const notes = readFileSync(new URL('../../THIRD-PARTY-NOTICES.md', import.meta.url), 'utf8');

test('the_symbols_font_is_in_the_notices_with_every_licence_and_its_files_exist', () => {
  assert.match(notes, /Symbols Nerd Font/);
  for (const l of ['MIT', 'CC BY 4.0', 'Apache 2.0', 'Left out: Font Logos (no licence']) assert.ok(notes.includes(l), l);
  for (const f of ['symbols-core.woff2', 'symbols-mdi.woff2', 'LICENSE.md']) assert.ok(existsSync(new URL(`../../packages/ui/src/terminal/fonts/${f}`, import.meta.url)), f);
});
