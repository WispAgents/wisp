// Audit E #2 (re-checked in audit G tasks): a phone's side sheet at 360 px with a pinned tab has more tabs than room. The
// strip must stay inside the screen and scroll (QA r7 #5): the row (`panel.tabs`) is the scroller, with no minimum width
// of its own, and the tab list inside it keeps its full width, so a swipe brings Thread in; the end it can still scroll
// to fades (Tabs.tsx markEnds). Checked by a touch swipe in Chrome at 360 (iOS and Android stubs); the looks' stylesheets
// say so here (the test DOM draws no layout).
import { expect, test } from 'vitest';

const sheets = Object.entries(import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true })).map(([p, raw]) => ({
  look: p.replace(/^.*\/|\.css$/g, ''),
  css: raw.replace(/\/\*[^]*?\*\//g, ''),
}));

for (const look of ['android', 'ios']) {
  test(`the_${look}_side_sheets_tab_row_scrolls_inside_the_screen_audit_e_2`, () => {
    const css = sheets.find((s) => s.look === look)?.css ?? '';
    const sheet = `:root\\[data-platform='${look}'\\] \\[data-part~='phone\\.side\\.sheet'\\] \\[data-kit='tabs'\\]\\[data-part='panel\\.tabs'\\]`;
    expect(css).toMatch(new RegExp(`${sheet} \\{[^}]*overflow-x: auto;`));
    expect(css).toMatch(new RegExp(`${sheet} > \\[data-kit='tab-list'\\] \\{[^}]*width: max-content;`));
    expect(css).toMatch(new RegExp(`${sheet}\\[data-scroll-end\\] \\{[^}]*mask-image`));
  });
}
