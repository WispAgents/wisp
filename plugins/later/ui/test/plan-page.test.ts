import { expect, test } from 'vitest';
import { planPage } from '../src/ui/plan-page.ts';
import { planSub } from '../src/format.ts';
import type { Plan } from '../src/types.ts';

const task = (o: Partial<Plan> = {}): Plan => ({ id: 'p1', kind: 'task', setAt: 0, at: 3_600_000 * 3, body: { prompt: 'fix the flaky test\nand more', agent: 'claude', cwd: '/w/api' }, ...o });

test('the_plan_page_says_when_it_starts_what_it_runs_on_and_whether_the_box_is_saved', () => {
  const p = planPage(task(), undefined, 0);
  expect([p.title, p.dirty, p.empty, p.hint, p.project]).toEqual(['fix the flaky test', false, false, 'wisp.later.plan.hint.task', 'api']);
  expect(p.when).toMatch(/^Starts /);
  // typing makes it dirty; clearing it says a task needs a prompt
  expect(planPage(task(), 'fix it now', 0)).toMatchObject({ text: 'fix it now', dirty: true, hint: 'wisp.later.plan.hint.dirty' });
  expect(planPage(task(), '  ', 0)).toMatchObject({ empty: true, hint: 'wisp.later.plan.hint.empty' });
  // same text with spaces around isn't a change
  expect(planPage(task(), ' fix the flaky test\nand more \n', 0).dirty).toBe(false);
  // a draft, a task after another, a reminder
  expect(planPage(task({ draft: true }), undefined, 0).when).toBe('Draft');
  expect(planPage(task({ after: { id: 's', title: 'Ship v2' } }), undefined, 0).when).toBe('After “Ship v2”');
  const r = planPage({ id: 'r', kind: 'reminder', setAt: 0, at: 60_000, text: 'call Lena' }, undefined, 0);
  expect([r.title, r.hint, r.runsOn]).toEqual(['call Lena', 'wisp.later.plan.hint.reminder', 'Reminder']);
  expect(r.when).toMatch(/^At /);
});

// audit doc23b: a plan whose start kept failing waits in Drafts and says so, not "was waiting for …"
test('a_plan_whose_start_kept_failing_says_so_under_its_title', () => {
  const lost: Plan = { id: 'p1', kind: 'task', setAt: 0, draft: true, body: { prompt: 'fix it', agent: 'claude' }, lost: { id: 'p1', title: 'fix it', why: 'failed' } };
  expect(planSub(lost, '')).toMatch(/^Couldn’t start at its time \(tried 3 times\) · /);
  expect(planSub(task({ draft: true, lost: { id: 's', title: 'Ship v2', why: 'deleted' } }), '')).toMatch(/^Was waiting for “Ship v2”, deleted/);
});
