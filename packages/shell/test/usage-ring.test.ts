// UX 2026-11-18 desktop #8: Home's usage bar is gone; the harness chip carries a small ring once the picked harness's
// fullest plan window passes 80% (warn), past 95% it reads as nearly out (bad). Under 80%: no ring.
import { expect, test } from 'vitest';
import { ringOf, usageRing } from '../src/frame/runner.ts';

test('the_harness_chip_shows_a_usage_ring_only_past_80_percent', () => {
  expect(usageRing(undefined)).toBeUndefined();
  expect(usageRing({ label: 'Weekly', used: 0.79 })).toBeUndefined();
  expect(usageRing({ label: 'Weekly', used: 0.8 })).toEqual({ pct: 80, tone: 'warn', label: 'Weekly' });
  expect(usageRing({ label: '5-hour', used: 0.967 })).toEqual({ pct: 96, tone: 'bad', label: '5-hour' });
  expect(usageRing({ label: 'Weekly', used: 1.4 })).toEqual({ pct: 100, tone: 'bad', label: 'Weekly' });
  expect(usageRing({ label: 'Weekly', used: Number.NaN })).toBeUndefined();
});

// Home and the task page read the same: the picked account's windows only ('' : every account), the fullest one
test('the_ring_reads_the_picked_accounts_fullest_window', () => {
  const l = [
    { account: 'work', windows: [{ label: 'Weekly', used: 0.97 }] },
    { account: 'me', windows: [{ label: 'Weekly', used: 0.1 }, { label: '5-hour', used: 0.85 }] },
  ];
  expect(ringOf(l, '')).toEqual({ pct: 97, tone: 'bad', label: 'Weekly' });
  expect(ringOf(l, 'me')).toEqual({ pct: 85, tone: 'warn', label: '5-hour' });
  expect(ringOf(l, 'gone')).toBeUndefined();
  expect(ringOf(undefined, '')).toBeUndefined();
});
