// Owner 2026-11-21: on the phone app a long task title in the task page's bar broke onto a second line. The page bar's
// title (Android's M3 medium large title, iOS's large title) is one line with an ellipsis on both platforms.
import { expect, test } from 'vitest';
import ANDROID from '../src/css/platform/android.css?raw';
import IOS from '../src/css/platform/ios.css?raw';

test.each([['android', ANDROID], ['ios', IOS]])('the_page_bars_title_is_one_line_with_an_ellipsis_on_%s', (look, css) => {
  for (const kit of ['app-bar-title', 'app-bar-large-title']) {
    const at = css.indexOf(`:root[data-platform='${look}'] [data-kit='${kit}'] {`);
    if (at < 0) continue;
    const rule = css.slice(at, css.indexOf('}', at));
    expect(rule, `${look} ${kit}`).toMatch(/white-space:\s*nowrap/);
    expect(rule, `${look} ${kit}`).toMatch(/text-overflow:\s*ellipsis/);
    expect(rule, `${look} ${kit}`).not.toMatch(/line-clamp/);
  }
  expect(css).toContain(`:root[data-platform='${look}'] [data-kit='app-bar-large-title'] {`);
});
