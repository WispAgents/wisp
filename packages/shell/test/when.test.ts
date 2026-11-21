import { expect, test } from 'vitest';
import { WHENS, whenAt, whenMenu } from '../src/frame/pickers.ts';

const t = (k: string, v?: Record<string, unknown>): string => (v ? `${k}:${JSON.stringify(v)}` : k);

test('start_later_never_offers_a_time_already_gone_v1_time_menu', () => {
  // QA r1 #6: at 21:43 the menu offered "This evening, 18:00"
  const late = new Date(2026, 10, 17, 21, 43).getTime();
  for (const w of WHENS) expect(whenAt(w, late)).toBeGreaterThan(late);
  // v1 later.js timeMenu: In 10 min, In 30 min, In 1 hour, In 3 hours, Tomorrow morning (9:00)
  expect(WHENS).toEqual(['10m', '30m', 'hour', '3h', 'morning']);
  expect(whenAt('10m', late) - late).toBe(10 * 60_000);
  expect(whenAt('3h', late) - late).toBe(3 * 3_600_000);
  const morning = new Date(whenAt('morning', late));
  expect([morning.getDate(), morning.getHours(), morning.getMinutes()]).toEqual([18, 9, 0]);
});

test('each_start_later_row_says_the_time_it_means', () => {
  const now = new Date(2026, 10, 17, 9, 5).getTime();
  const items = whenMenu(t, now);
  expect(items.map((i) => ('value' in i ? i.value : ''))).toEqual(['10m', '30m', 'hour', '3h', 'morning']);
  // the time column (v1 .lm-t): 09:15 for In 10 min
  expect('hint' in items[0]! ? items[0].hint : undefined).toBe(new Date(now + 600_000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }));
});
