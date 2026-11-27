// Merge risk on the PR row (ADR-0140): the strip's button shows the worst of the task's open PRs (several, across
// repos), each chip its own word, Merge's confirm turns red for a high one, and nothing shows while the setting is off.
import { expect, test } from 'vitest';
import { t } from '../src/i18n.ts';
import { mergePlan } from '../src/model/merge.ts';
import { openPrs } from '../src/model/pr.ts';
import { chipRisk, riskStrip, riskWarnings } from '../src/model/risk.ts';
import type { RiskVerdict, ShipSession } from '../src/types.ts';

const ctx = { reviewOn: false, agentName: () => undefined };
const pr = (n: number, head: string) => ({ number: n, url: `https://github.com/o/r/pull/${n}`, state: 'OPEN' as const, head });
const v = (n: number, head: string, more: Partial<RiskVerdict>): RiskVerdict => ({ url: pr(n, head).url, repo: 'r', number: n, head, status: 'done', at: 1, current: true, ...more });

const two = (risk: RiskVerdict[], riskOn = true): ShipSession => ({
  id: 's1',
  agent: 'claude',
  project: 'app',
  pr: pr(1, 'a'),
  prs: [{ repo: 'lib', pr: pr(2, 'b') }],
  riskOn,
  risk,
});

test('off_or_without_an_open_pr_the_strip_says_nothing', () => {
  const s = two([v(1, 'a', { level: 'high' })], false);
  expect(riskStrip(t, s, openPrs(s))).toBeUndefined();
  expect(chipRisk(t, s, pr(1, 'a'))).toBeUndefined();
  expect(riskWarnings(t, s, openPrs(s)).high).toEqual([]);
  const on = two([]);
  expect(riskStrip(t, on, [])).toBeUndefined();
});

test('the_strip_shows_the_worst_open_pr_and_every_prs_line', () => {
  const s = two([v(1, 'a', { level: 'low', score: 10, summary: 'docs only' }), v(2, 'b', { level: 'high', score: 82, summary: 'drops a column' })]);
  const r = riskStrip(t, s, openPrs(s));
  expect(r?.kind).toBe('high');
  expect(r?.label).toBe('High risk');
  expect(r?.title).toContain('app #1 Low risk (10/100): docs only');
  expect(r?.title).toContain('lib #2 High risk (82/100): drops a column');
  expect(chipRisk(t, s, pr(1, 'a'))).toBe('low risk');
  expect(chipRisk(t, s, pr(2, 'b'))).toBe('high risk');
});

test('a_high_one_outweighs_one_still_being_judged_and_two_high_are_counted', () => {
  const judging = two([v(1, 'a', { status: 'running' }), v(2, 'b', { level: 'high' })]);
  expect(riskStrip(t, judging, openPrs(judging))?.kind).toBe('high');
  const both = two([v(1, 'a', { level: 'high' }), v(2, 'b', { level: 'high' })]);
  expect(riskStrip(t, both, openPrs(both))?.label).toBe('High risk · 2');
  const allLow = two([v(1, 'a', { level: 'low' }), v(2, 'b', { level: 'low' })]);
  expect(riskStrip(t, allLow, openPrs(allLow))?.kind).toBe('low');
});

test('a_push_since_the_verdict_reads_as_not_judged_and_the_button_judges', () => {
  const s = two([v(1, 'a', { level: 'low', current: false }), v(2, 'b', { status: 'failed', why: 'gh failed' })]);
  const r = riskStrip(t, s, openPrs(s));
  expect(r?.kind).toBe('failed');
  expect(r?.toJudge).toBe(2);
  expect(r?.title).toContain('lib #2 Risk unknown: gh failed');
  expect(r?.title).toContain('Click to have another harness judge it');
  expect(chipRisk(t, s, pr(1, 'a'))).toBeUndefined();
});

test('merge_turns_red_for_a_high_risk_pr_and_warns_for_a_medium_one', () => {
  const high = two([v(1, 'a', { level: 'medium', summary: 'wide refactor' }), v(2, 'b', { level: 'high', summary: 'drops a column' })]);
  const p = mergePlan(t, high, ctx);
  expect(p?.danger).toBe(true);
  expect(p?.lines).toContain('Merge risk: lib #2 High risk: drops a column');
  expect(p?.lines).toContain('Merge risk: app #1 Medium risk: wide refactor');
  const medium = two([v(1, 'a', { level: 'medium' }), v(2, 'b', { level: 'low' })]);
  expect(mergePlan(t, medium, ctx)?.danger).toBe(false);
});
