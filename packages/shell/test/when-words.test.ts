// QA r6 desktop #2: Start later says when in v1's words (launcher.js paintWhen, later.js whenTip): the chip "Starts at
// 07:52" (today) or "Starts Wed 09:00", its tip "In 10 min · Tue 18 Nov 07:52", Send "Start at 07:52". Never the menu's
// "In 10 min" mid-phrase, never a raw date-time with seconds.
import { expect, test } from 'vitest';
import { startsAt, whenTip } from '../src/frame/hold.ts';

const now = new Date(2026, 10, 18, 7, 42, 59).getTime(); // Wed 18 Nov 2026 07:42:59, local

test('start_later_names_the_time_in_v1s_words', () => {
  expect(startsAt(now + 600_000, now)).toBe('at 07:52');
  expect(startsAt(new Date(2026, 10, 19, 9, 0).getTime(), now)).toBe('Thu 09:00');
  expect(whenTip(now + 600_000, now)).toBe('In 10 min · Wed 18 Nov 07:52');
  expect(whenTip(now + 3 * 3_600_000, now)).toBe('In 3 h · Wed 18 Nov 10:42');
  expect(whenTip(new Date(2026, 10, 21, 9, 0).getTime(), now)).toBe('In 3 days · Sat 21 Nov 09:00');
  // nothing with seconds
  expect(whenTip(now + 600_000, now)).not.toMatch(/\d\d:\d\d:\d\d/);
});
