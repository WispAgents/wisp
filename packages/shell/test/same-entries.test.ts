// The open task's menu stays the same object while what it says doesn't change (perf r1 #12, #15: a streamed token
// rebuilt the closed More menu's 30 rows), and changes as soon as one row says or does something else.
import { expect, test } from 'vitest';
import type { MenuEntry } from '@wisp/ui';
import { sameEntries } from '../src/frame/same-entries.ts';

const menu = (over: Partial<Record<string, unknown>> = {}): MenuEntry[] => [
  { value: 'rename', label: 'Rename', icon: 'pen', keys: 'M' },
  { kind: 'separator' },
  { kind: 'group', id: 'lean', label: 'Lean runs', items: [{ value: 'lean:follow', label: 'As Settings say', checked: true }, { value: 'lean:on', label: 'On' }] },
  { kind: 'submenu', value: 'copy', label: 'Copy', items: [{ value: 'copy-id', label: 'Copy task ID', icon: 'copy' }] },
  { value: 'archive', label: 'Archive', icon: 'archive', keys: 'A', ...over } as MenuEntry,
  { value: 'delete', label: 'Delete', danger: true },
];

test('a_menu_made_again_with_the_same_rows_is_the_same_menu', () => {
  expect(sameEntries(menu(), menu())).toBe(true);
});

test('any_row_that_says_or_does_something_else_makes_a_new_menu', () => {
  expect(sameEntries(menu(), menu({ label: 'Unarchive' }))).toBe(false);
  expect(sameEntries(menu(), menu({ disabled: true }))).toBe(false);
  expect(sameEntries(menu(), menu({ checked: true }))).toBe(false);
  expect(sameEntries(menu(), menu({ value: 'unarchive' }))).toBe(false);
  expect(sameEntries(menu(), menu().slice(0, -1))).toBe(false);
  const lean = menu();
  const g = lean[2] as unknown as { items: readonly { checked?: boolean }[] };
  const on = [...lean.slice(0, 2), ({ ...lean[2], items: [{ ...g.items[0], checked: false }, { ...g.items[1], checked: true }] }) as unknown as MenuEntry, ...lean.slice(3)];
  expect(sameEntries(menu(), on)).toBe(false); // a group's check moved
  const copy = menu();
  const sub = [...copy.slice(0, 3), { kind: 'submenu', value: 'copy', label: 'Copy', items: [{ value: 'copy-id', label: 'Copy task ID', icon: 'copy' }, { value: 'copy-cmd', label: 'Copy resume command' }] } as MenuEntry, ...copy.slice(4)];
  expect(sameEntries(menu(), sub)).toBe(false); // a submenu's rows
});
