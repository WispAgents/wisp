// @vitest-environment happy-dom
// QA audit D #4: a scheduled new task's row (v1 later.js planRows) is a feed row like a task's, its clock in the dot
// column, so its title lines up with the task rows on a phone, and it says what it is and its project on one line.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { Env } from '../src/ui/env.ts';
import { PlanRow } from '../src/ui/OtherRows.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

test('a_scheduled_new_tasks_row_is_a_feed_row_with_one_line_under_its_title_v1_plan_row', () => {
  const env = { shell: { t: (k: string) => (k === 'shell-list.plan.task' ? 'Scheduled task' : k), extensions: () => () => [] }, client: { navigate: () => undefined }, now: () => 0 };
  const host = document.createElement('div');
  document.body.append(host);
  const plan = { id: 'p1', kind: 'task', title: 'Check the deploy', at: 3_600_000, project: 'app' } as never;
  const stop = render(
    () => (
      <Env.Provider value={env as never}>
        <PlanRow plan={plan} />
      </Env.Provider>
    ),
    host,
  );
  const row = host.querySelector('[data-kit="list-row"]');
  expect(row?.getAttribute('data-layout'), 'the task rows’ own layout').toBe('feed');
  const sub = host.querySelector('[data-kit="list-row-sub"]');
  expect(sub?.textContent).toBe('Scheduled task · app');
  stop();
});
