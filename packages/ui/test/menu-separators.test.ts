// A menu never draws a separator first, last or twice (owner, 2026-11-17: a line over Machines' ⋯ Sign out).
import { expect, test } from 'vitest';
import { tidySeparators } from '../src/controls/menu-entries.ts';

const sep = { kind: 'separator' as const };
const item = (value: string) => ({ value, label: value });

test('separators only between items', () => {
  expect(tidySeparators([sep, item('remove')])).toEqual([item('remove')]);
  expect(tidySeparators([item('a'), sep, sep, item('b'), sep])).toEqual([item('a'), sep, item('b')]);
  expect(tidySeparators([sep])).toEqual([]);
});
