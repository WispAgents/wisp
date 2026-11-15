// v1 #796: the follow-ups over the reply box are one compact list. Past 3, the first 2 show with "N more scheduled ·
// next 14:52", which opens the rest; who set it and when in words are the row's tooltip.
import { expect, test } from 'vitest';
import { followUpTip } from '../src/format.ts';
import { FOOT_MAX, footRows } from '../src/rules.ts';

const at = (n: number): { id: string; at: number } => ({ id: `l${n}`, at: n * 60_000 });

test('three_follow_ups_all_show_with_no_more_row_v1_796', () => {
  const all = [at(1), at(2), at(3)];
  expect(FOOT_MAX).toBe(3);
  expect(footRows(all, false)).toEqual({ shown: all });
});

test('past_three_the_first_two_show_and_the_more_row_names_the_rest_and_the_next_v1_796', () => {
  const all = [at(1), at(2), at(3), at(4), at(5)];
  const closed = footRows(all, false);
  expect(closed.shown.map((l) => l.id)).toEqual(['l1', 'l2']);
  expect(closed.more).toEqual({ rest: 3, next: 3 * 60_000 });
  const open = footRows(all, true);
  expect(open.shown).toEqual(all);
  expect(open.more?.rest).toBe(3);
});

test('who_set_it_and_when_in_words_go_in_the_tooltip_v1_796', () => {
  const now = Date.UTC(2026, 10, 4, 14, 14);
  const tip = followUpTip({ id: 'l1', at: now + 18 * 60_000, text: 'check CI', by: 'agent', setAt: 0 }, now);
  expect(tip.split('\n')[0]).toMatch(/^Follow-up · In 18 min · /);
  expect(tip.split('\n')[1]).toBe('Set by the agent');
  const held = followUpTip({ id: 'l2', at: now + 60_000, text: 'go on', by: 'agent', setAt: 0, remind: true, held: { why: 'past 75%', until: 0 } }, now);
  expect(held).toMatch(/^Reminder · /);
  expect(held.split('\n')[1]).toBe('When your plan resets. Waits for your plan: past 75%');
});
