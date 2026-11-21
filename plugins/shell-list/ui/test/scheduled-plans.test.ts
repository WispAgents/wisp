// Audit B #6 (re-checked in audit C): Start later's plan was in `later.plans` but the list showed no Scheduled row. The
// row as wisp.later sends it on its topic (`later_plan`: `{id, body}`, the plan in its body, ranges kept with it since
// ADR-0055) is a Scheduled row with its time, under All and under its project, beside a reminder, soonest first.
import { expect, test } from 'vitest';
import { sections, ALL } from '../src/model/sections.ts';
import { flatten, itemKey } from '../src/model/flat.ts';
import { listPlan } from '../src/model/wire.ts';
import type { Plan, Row } from '../src/types.ts';

const at = Date.UTC(2026, 10, 20, 9);
const rows: unknown[] = [
  // New task › Start later in proj: a worktree of its own, its project, the composer's picks (later.ts `send`)
  {
    id: '01a1f3ce',
    body: {
      id: '01a1f3ce',
      kind: 'task',
      setAt: at - 86_400_000,
      at,
      title: 'B6 start later from the UI',
      body: { prompt: 'B6 start later from the UI', cwd: '/tmp/qa/proj', projectId: 'proj', worktree: true, agent: 'claude', mode: 'auto', envHide: {} },
    },
  },
  { id: '01a1f3cd', body: { id: '01a1f3cd', kind: 'reminder', setAt: at - 1, at: at + 3_600_000, text: 'check the deploy' } },
];

test('start_laters_plan_is_a_scheduled_row_with_its_time_under_all_and_its_project_audit_b_6', () => {
  const plans = rows.map(listPlan).filter((p): p is Plan => p !== undefined) as Row<Plan>[];
  expect(plans.map((p) => [p.kind, p.title, p.at, p.projectId ?? null])).toEqual([
    ['task', 'B6 start later from the UI', at, 'proj'],
    ['reminder', 'check the deploy', at + 3_600_000, null],
  ]);
  const all = sections([], ALL, plans);
  expect(all.scheduled.map((e) => (e.kind === 'plan' ? e.plan.title : '?'))).toEqual(['B6 start later from the UI', 'check the deploy']);
  // the list draws Scheduled's heading and its rows (open by default), with nothing else in it
  const keys = flatten(all, { toggled: new Set(), split: false }).map(itemKey);
  expect(keys).toEqual(['h:scheduled', 'p:01a1f3ce', 'p:01a1f3cd']);
  // under its project: still there; under another: not
  expect(sections([], { project: 'proj' }, plans).scheduled.map((e) => e.key)).toContain('p:01a1f3ce');
  expect(sections([], { project: 'other' }, plans).scheduled.map((e) => e.key)).not.toContain('p:01a1f3ce');
});
