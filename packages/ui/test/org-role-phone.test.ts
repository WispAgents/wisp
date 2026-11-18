// QA phone r3 #2: on Android 360 each Org card is ~150 px wide and "Crew member" broke onto two lines under the name.
// v1's role may take two lines on desktop (a role is a phrase); on a phone card it's one line with an ellipsis.
import { expect, test } from 'vitest';
import ANDROID from '../src/css/platform/android.css?raw';
import IOS from '../src/css/platform/ios.css?raw';

test.each([['android', ANDROID], ['ios', IOS]])('the_org_cards_role_is_one_line_on_%s', (look, css) => {
  const at = css.indexOf(`:root[data-platform='${look}'] [data-kit='org-who'] > small`);
  expect(at, 'a phone rule for the role').toBeGreaterThan(-1);
  const rule = css.slice(at, css.indexOf('}', at));
  expect(rule).toMatch(/white-space:\s*nowrap/);
  expect(rule).toMatch(/text-overflow:\s*ellipsis/);
});
