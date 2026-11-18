// QA r15 #5: a phone sheet's or dialog's ✕ is a finger's target: 44 pt on iOS, 48 dp on Android (--tap-min), its glyph
// its own size. The looks' stylesheets say so (the box is measured in the real app; the test DOM draws no layout).
import { expect, test } from 'vitest';

const sheets = Object.entries(import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true })).map(([p, raw]) => ({
  look: p.replace(/^.*\/|\.css$/g, ''),
  css: raw.replace(/\/\*[^]*?\*\//g, ''),
}));

for (const look of ['android', 'ios']) {
  test(`the_${look}_look_gives_a_sheet_and_a_dialog_close_the_touch_target`, () => {
    const css = sheets.find((s) => s.look === look)?.css ?? '';
    const P = `:root\\[data-platform='${look}'\\]`;
    const rule = new RegExp(
      `${P} \\[data-kit='dialog'\\] > \\[data-kit='dialog-head'\\] > \\[data-kit='icon-button'\\],\\s*${P} \\[data-kit='sheet'\\] > \\[data-kit='dialog-head'\\] > \\[data-kit='icon-button'\\] \\{[^}]*min-width: var\\(--tap-min\\);[^}]*min-height: var\\(--tap-min\\);`,
    );
    expect(css).toMatch(rule);
  });
}
