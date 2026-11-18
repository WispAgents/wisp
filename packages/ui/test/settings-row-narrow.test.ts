// QA phone r4 #2: the phone Machines sheet drew "Online", "Disconnect" and "wisp v1 (0.58.0)" over each other and the
// ⋯ over the Update icon, its icon above its name; at 360 "This computer" was cut to "This co…". Two kit rules:
// - a row with an icon tile and a wide control, on a narrow screen: the tile stays beside the words, the control and
//   what's below go under the words, each on a line of its own (its `below` asked for column 2 of a one-column grid);
// - a row's control drops under its words on a narrow screen too, not only in a narrow `settings` container (a phone
//   dialog isn't one).
import { expect, test } from 'vitest';
import CSS from '../src/css/composites.css?raw';

const block = (head: string): string => {
  const at = CSS.indexOf(head);
  expect(at, head).toBeGreaterThan(-1);
  let depth = 0;
  for (let i = CSS.indexOf('{', at); i < CSS.length; i++) {
    if (CSS[i] === '{') depth++;
    if (CSS[i] === '}' && --depth === 0) return CSS.slice(at, i + 1);
  }
  return '';
};

test('a_wide_tile_row_on_a_narrow_screen_keeps_its_tile_beside_its_words', () => {
  const narrow = block('@media (max-width: 520px)');
  expect(narrow).toMatch(/\[data-kit='settings-row'\]\[data-wide\]\[data-tile\]\s*\{[^}]*grid-template-columns:\s*auto minmax\(0, 1fr\)/);
  expect(narrow).toMatch(/\[data-kit='settings-row'\]\[data-wide\]\[data-tile\] > \[data-kit='settings-row-control'\]\s*\{[^}]*grid-column:\s*2/);
});

test('a_rows_control_drops_under_its_words_on_a_narrow_screen_too', () => {
  const narrow = block('@media (max-width: 520px)');
  expect(narrow).toMatch(/\[data-kit='settings-row'\]\[data-layout='wrap'\]\s*\{[^}]*display:\s*flex/);
});
