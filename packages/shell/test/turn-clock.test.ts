// The turn clock follows the app's locale (v1 turnTime used `Intl.DateTimeFormat([], …)`), never a forced en-GB.
import { expect, test } from 'vitest';
import { turnClock } from '../src/frame/turn-clock.ts';

const at = new Date(2026, 10, 15, 14, 5).getTime();

test('turn_clock_uses_the_app_locale_not_en_gb', () => {
  expect(turnClock(at)).toBe(new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(at));
  expect(turnClock(at, 'en-US')).toMatch(/^02:05\s?PM$/);
  expect(turnClock(at, 'en-GB')).toBe('14:05');
});
