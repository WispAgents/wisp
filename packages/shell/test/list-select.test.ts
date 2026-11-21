// ADR-0065 step 13: the phone Tasks list's selection mode (design/mobile.md §A.4): enter, toggle, Select all, rows that
// leave the list. (Android's leaving with the last row unpicked is the kit's SelectionBar: platform-pages tests it.)
import { expect, test } from 'vitest';
import { NONE, allSelected, pruneSelect, selectAll, startSelect, toggleSelect } from '../src/frame/list-select.ts';

test('select_from_a_rows_menu_starts_with_that_row_and_from_the_bar_with_none', () => {
  expect(startSelect('a')).toEqual({ on: true, keys: ['a'] });
  expect(startSelect()).toEqual({ on: true, keys: [] });
});

test('a_tap_toggles_a_row_in_selection_and_does_nothing_outside_it', () => {
  const s = toggleSelect(startSelect('a'), 'b');
  expect(s.keys).toEqual(['a', 'b']);
  expect(toggleSelect(s, 'a').keys).toEqual(['b']);
  expect(toggleSelect(NONE, 'a')).toBe(NONE);
});

test('unpicking_the_last_row_keeps_selection_on_until_its_bar_leaves', () => {
  expect(toggleSelect(startSelect('a'), 'a')).toEqual({ on: true, keys: [] });
});

test('select_all_picks_every_task_the_list_shows_and_again_picks_none', () => {
  const all = selectAll(startSelect('b'), ['a', 'b', 'c']);
  expect(all.keys).toEqual(['a', 'b', 'c']);
  expect(allSelected(all, ['a', 'b', 'c'])).toBe(true);
  expect(selectAll(all, ['a', 'b', 'c']).keys).toEqual([]);
  expect(allSelected(startSelect(), [])).toBe(false);
});

test('rows_that_leave_the_list_leave_the_selection', () => {
  const s = { on: true, keys: ['a', 'b'] };
  expect(pruneSelect(s, ['b', 'c']).keys).toEqual(['b']);
  expect(pruneSelect(s, ['a', 'b'])).toBe(s);
  expect(pruneSelect(s, ['c'])).toEqual({ on: true, keys: [] });
  expect(pruneSelect(NONE, ['c'])).toBe(NONE);
});
