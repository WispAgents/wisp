// Audit G tasks #4: on a phone the answer buttons of a question card, a setup card and a factory's question (Allow,
// Deny, Show all, Approve, Stop, an option) were 28 px (iOS) and 32 px (Android). They are touch targets like the held
// card's (audit F tasks #4): 44 pt on iOS, 48 dp on Android (--tap-min). The looks' stylesheets say so (the box is
// measured in the real app; the test DOM draws no layout).
import { expect, test } from 'vitest';

const sheets = Object.entries(import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true })).map(([p, raw]) => ({
  look: p.replace(/^.*\/|\.css$/g, ''),
  css: raw.replace(/\/\*[^]*?\*\//g, ''),
}));

for (const look of ['android', 'ios']) {
  test(`the_${look}_look_gives_a_cards_answer_buttons_the_touch_target_audit_g_tasks_4`, () => {
    const css = sheets.find((s) => s.look === look)?.css ?? '';
    const rule = new RegExp(`:root\\[data-platform='${look}'\\] \\[data-part~='task\\.ask'\\] \\[data-kit='wrap'\\] > \\[data-kit='button'\\] \\{[^}]*min-height: var\\(--tap-min\\);`);
    expect(css).toMatch(rule);
  });
}

// Audit H tasks (G #4 partly fixed): the working panel's Stop (`task.stop`, on every running or asking task) was still
// 68×28 on iOS and 81×32 on Android.
for (const look of ['android', 'ios']) {
  test(`the_${look}_look_gives_the_working_panels_stop_the_touch_target_audit_h_tasks`, () => {
    const css = sheets.find((s) => s.look === look)?.css ?? '';
    const rule = new RegExp(`:root\\[data-platform='${look}'\\] \\[data-kit='button'\\]\\[data-part~='task\\.stop'\\] \\{[^}]*min-height: var\\(--tap-min\\);`);
    expect(css).toMatch(rule);
  });
}
