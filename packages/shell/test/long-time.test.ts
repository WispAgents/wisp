// A row's and a turn's hover time (v1's title: Sun, 8 Nov 2026, 03:28), from formatters made once (perf r1 #6: two
// DateTimeFormats built per call, per row and per turn, on every open of a task).
import { expect, test } from 'vitest';
import { longTime } from '../src/frame/rows-view.ts';

test('the_hover_time_reads_as_v1s_and_stays_the_same_on_every_call', () => {
  const at = new Date(2026, 10, 8, 3, 28).getTime();
  expect(longTime(at)).toBe('Sun, 8 Nov 2026, 03:28');
  expect(longTime(at)).toBe(longTime(at));
  expect(longTime(new Date(2026, 0, 1, 23, 5).getTime())).toBe('Thu, 1 Jan 2026, 23:05');
});
