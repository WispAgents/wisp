import { expect, test } from 'vitest';
import { stewardAction, stewardArg, stewardLine, stewardStart } from '../src/model/steward.ts';
import type { T } from '../src/i18n.ts';
import type { PrSummary, ShipSession, Steward } from '../src/types.ts';

const t = ((k: string, v?: Readonly<Record<string, string | number>>): string => `${k}${v ? JSON.stringify(v) : ''}`) as T;
const pr = (state: 'OPEN' | 'MERGED' = 'OPEN'): PrSummary => ({ number: 7, url: 'u', state }) as unknown as PrSummary;
const row = (steward?: Steward, state: 'OPEN' | 'MERGED' = 'OPEN'): ShipSession => ({ id: 't', agent: 'claude', pr: pr(state), ...(steward && { steward }) });

test('the_ship_dialog_box_follows_the_task_then_the_setting_and_unticking_sends_off_only_when_one_is_on', () => {
  expect(stewardStart(undefined, 'off')).toEqual({ on: false, review: false });
  expect(stewardStart(undefined, 'review')).toEqual({ on: true, review: true });
  expect(stewardStart({ mode: 'fix', step: 'fixing' }, 'review')).toEqual({ on: true, review: false });
  // a stopped one doesn't hold the box: the setting does
  expect(stewardStart({ mode: 'review', step: 'stopped' }, 'off')).toEqual({ on: false, review: false });
  expect([stewardArg(true, false, undefined), stewardArg(true, true, undefined), stewardArg(false, true, undefined), stewardArg(false, false, { mode: 'fix', step: 'green' })]).toEqual(['fix', 'review', undefined, 'off']);
});

test('the_pr_row_says_what_the_steward_does_and_offers_one_control', () => {
  expect(stewardLine(t, row({ mode: 'fix', step: 'fixing', causes: ['checks', 'threads'], passes: { checks: 1, threads: 1 } }))).toBe(
    'wisp.ship.steward.fixing{"what":"wisp.ship.steward.cause.checks, wisp.ship.steward.cause.threads","n":2}',
  );
  expect(stewardLine(t, row({ mode: 'review', step: 'green', reviewer: 'codex · gpt-6.2' }))).toBe('wisp.ship.steward.greenBy{"who":"codex · gpt-6.2"}');
  expect(stewardLine(t, row({ mode: 'fix', step: 'stopped', why: 'Tried 3 times' }))).toBe('wisp.ship.steward.stopped{"why":"Tried 3 times"}');
  expect(stewardLine(t, row({ mode: 'review', step: 'watching' }))).toBe('wisp.ship.steward.watchingReview');
  expect(stewardLine(t, row())).toBeUndefined();
  // merged: the line goes (a stop's reason stays), and there's nothing to steward
  expect(stewardLine(t, row({ mode: 'fix', step: 'green' }, 'MERGED'))).toBeUndefined();
  expect(stewardLine(t, row({ mode: 'fix', step: 'stopped', why: 'The PR merged.' }, 'MERGED'))).toBe('wisp.ship.steward.stopped{"why":"The PR merged."}');
  expect([stewardAction(row()), stewardAction(row({ mode: 'fix', step: 'fixing' })), stewardAction(row({ mode: 'fix', step: 'stopped' })), stewardAction(row(undefined, 'MERGED'))]).toEqual(['start', 'stop', 'again', undefined]);
});
