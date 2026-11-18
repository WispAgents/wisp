// UX 2026-11-18 desktop #13: the Shell drew boxes (□) where a user's prompt has powerline / Nerd Font glyphs: the mono
// stack has none. Browsers fall back per glyph, so the Nerd Font families a user's own terminal uses come after it.
import { expect, test } from 'vitest';
import CSS from '../src/css/terminal.css?raw';

test('the_shells_font_falls_back_to_nerd_font_symbols_for_prompt_glyphs', () => {
  const rule = CSS.match(/\[data-kit='term-rows'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/var\(--mono\)/);
  expect(rule).toMatch(/'Symbols Nerd Font Mono'/);
  expect(rule).toMatch(/'MesloLGS NF'/);
});
