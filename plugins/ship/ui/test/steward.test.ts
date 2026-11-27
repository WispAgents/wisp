import { expect, test } from 'vitest';
import { stewardArg, stewardLine, stewardStart, stewardStatus, stewardSwitch } from '../src/model/steward.ts';
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
});

test('the_steward_says_green_only_for_the_head_it_judged_once_its_checks_ran', () => {
  // PR #996: green from a look at a PR with no checks listed yet, shown under "5 checks running"
  const at = (p: Partial<PrSummary>): ShipSession => ({ id: 't', agent: 'claude', pr: { ...pr(), head: 'h1', ...p }, steward: { mode: 'fix', step: 'green', head: 'h1' } });
  expect(stewardLine(t, at({ checks: { pass: 3, fail: 0, pending: 0 } }))).toBe('wisp.ship.steward.green');
  expect(stewardLine(t, at({ checks: { pass: 1, fail: 0, pending: 5 } }))).toBe('wisp.ship.steward.watching');
  expect(stewardLine(t, at({ checks: { pass: 0, fail: 0, pending: 0 }, waitingChecks: true }))).toBe('wisp.ship.steward.watching');
  expect(stewardLine(t, at({ head: 'h2', checks: { pass: 3, fail: 0, pending: 0 } })), 'a newer push').toBe('wisp.ship.steward.watching');
});

test('the_pr_row_switch_shows_on_any_open_pr_and_restarts_in_the_mode_it_had_else_the_setting', () => {
  // a PR the agent attached (wisp_pr) is the same row as a shipped one: no Steward yet, the switch is off
  expect(stewardSwitch(row(), 'off')).toEqual({ on: false, next: 'fix' });
  expect(stewardSwitch(row(), 'review'), 'Settings → Ship says review').toEqual({ on: false, next: 'review' });
  expect(stewardSwitch(row({ mode: 'fix', step: 'fixing' }), 'review')).toEqual({ on: true, next: 'off' });
  expect(stewardSwitch(row({ mode: 'review', step: 'green' }), 'off')).toEqual({ on: true, next: 'off' });
  // stopped: off, and on again keeps its mode over the setting
  expect(stewardSwitch(row({ mode: 'review', step: 'stopped' }), 'fix')).toEqual({ on: false, next: 'review' });
  expect(stewardSwitch(row(undefined, 'MERGED'), 'fix'), 'nothing to steward').toBeUndefined();
});

test('the_steward_row_says_its_status_without_repeating_steward', () => {
  // the row's title already says Steward: its status uses the bare keys, the same states as the line
  expect(stewardStatus(t, row({ mode: 'fix', step: 'fixing', causes: ['checks'], passes: { checks: 2 } }))).toBe('wisp.ship.steward.s.fixing{"what":"wisp.ship.steward.cause.checks","n":2}');
  expect(stewardStatus(t, row({ mode: 'review', step: 'watching' }))).toBe('wisp.ship.steward.s.watchingReview');
  expect(stewardStatus(t, row({ mode: 'fix', step: 'stopped', why: 'Tried 3 times' }))).toBe('wisp.ship.steward.s.stopped{"why":"Tried 3 times"}');
  expect(stewardStatus(t, row()), 'off: the row says what it would do instead').toBeUndefined();
  expect(stewardLine(t, row({ mode: 'fix', step: 'pushing' }))).toBe('wisp.ship.steward.pushing');
});
