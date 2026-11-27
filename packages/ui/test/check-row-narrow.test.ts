// QA r2 phone #7: on a phone (and in the desktop side panel) every PR check's ↗ logs icon dropped to a line of its own
// under the name, doubling each row: the narrow layout put the actions on a second grid row. They stay on the check's
// line, in their own last column; the name gives way (it ellipsizes).
import { expect, test } from 'vitest';
import CSS from '../src/css/ship.css?raw';

const narrow = (): string => {
  const at = CSS.indexOf('@container checks (max-width: 520px)');
  expect(at).toBeGreaterThan(-1);
  return CSS.slice(at, CSS.indexOf('\n  }\n', at));
};

test('a_narrow_check_row_keeps_its_actions_on_its_line', () => {
  const n = narrow();
  const actions = n.match(/\[data-kit='check-row-actions'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(actions).not.toMatch(/grid-row\s*:\s*2/);
  // the row is one line of columns at every width (#1003: v1's .pn-pr check row, actions in its last column), and the
  // narrow layout never gives it a second row
  expect(n).not.toMatch(/grid-template-rows|grid-row\s*:/);
  const row = CSS.match(/\[data-kit='check-row'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(row).toMatch(/grid-template-columns:\s*auto minmax\(0, 1fr\)( auto){3}/);
  const own = CSS.match(/\[data-kit='check-row-actions'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(own).toMatch(/grid-column:\s*5/);
});
