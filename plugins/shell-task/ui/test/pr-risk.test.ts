// The PR pane's merge risk section (ADR-0140): per picked PR, from the row's `risk` (under `summary` on a server row),
// hidden while the setting is off or the PR isn't open; a push since the verdict says so.
import { expect, test } from 'vitest';
import { riskSection } from '../src/side/risk.ts';

const verdict = (url: string, more: Record<string, unknown>) => ({ url, head: 'h', status: 'done', current: true, ...more });

test('each_pr_gets_its_own_verdict_and_off_hides_it', () => {
  const row = { id: 's1', summary: { riskOn: true, risk: [verdict('u1', { level: 'low' }), verdict('u2', { level: 'high', score: 90 })] } };
  expect(riskSection(row, 'u1', true).state).toBe('low');
  expect(riskSection(row, 'u2', true)).toMatchObject({ state: 'high', verdict: { score: 90 } });
  expect(riskSection(row, 'u3', true).state).toBe('none');
  expect(riskSection(row, 'u2', false).state).toBe('off');
  expect(riskSection({ id: 's1', summary: { risk: row.summary.risk } }, 'u2', true).state).toBe('off');
});

test('a_push_since_reads_stale_and_a_failed_judge_failed', () => {
  const flat = { riskOn: true, risk: [verdict('u1', { level: 'low', current: false }), verdict('u2', { status: 'failed', why: 'x' }), verdict('u3', { status: 'running' })] };
  expect(riskSection(flat, 'u1', true).state).toBe('stale');
  expect(riskSection(flat, 'u2', true).state).toBe('failed');
  expect(riskSection(flat, 'u3', true).state).toBe('running');
});
