// Audit B #18: Settings › Worktrees read "13 worktrees · 288 KB" for minutes while a look was still finding 41; the
// total says it's still looking while a look is under way.
import { expect, test } from 'vitest';
import { totalLine } from '../src/format.ts';
import type { Entry, WorktreesStatus } from '../src/types.ts';

const status = (looking: boolean): WorktreesStatus => ({ size: 3000, disk: null, looking, cleaning: null, last: null });
const list = [{ size: 1000 }, { size: 2000 }] as unknown as Entry[];

test('the_total_says_still_looking_while_a_look_is_under_way', () => {
  expect(totalLine(list, status(true))).toBe('2 worktrees · 3 KB · still looking…');
  expect(totalLine(list, status(false))).toBe('2 worktrees · 3 KB');
  expect(totalLine([], { ...status(true), size: null })).toBe('0 worktrees · still looking…');
});
