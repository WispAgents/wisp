// QA round 6 phone #14: the task page's status pills (UX 2026-11-18 phone #3, "under the title") sat at the very bottom,
// under the reply box: the pane lays its pushed page's bar out first with `order: -1`, and the pills, which follow the
// bar, kept their place after the page. They come first too (after the bar, in its order), on both looks.
import { expect, test } from 'vitest';
import IOS from '../src/css/platform/ios.css?raw';
import ANDROID from '../src/css/platform/android.css?raw';

const css = (s: string): string => s.replace(/\/\*[^]*?\*\//g, '');

test('the_status_pills_sit_under_the_bar_not_under_the_reply_box', () => {
  for (const [look, s] of [['ios', IOS], ['android', ANDROID]] as const) {
    const r = new RegExp(`:root\\[data-platform='${look}'\\] \\[data-kit='frame-pane'\\] > \\[data-kit='page-status'\\]\\s*\\{([^}]*)\\}`).exec(css(s))?.[1] ?? '';
    expect(r, look).toMatch(/order:\s*-1/);
  }
});
