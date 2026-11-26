// ⌘K / search opens a task by its entity key (owner, 2026-11-17: opening a task from search showed a blank page).
// The session list's rows carry bare ids; the task page opens by `<peer>:<id>`.
import { expect, test } from 'vitest';
import { taskItems } from '../src/palette/tasks.ts';

test('a_task_found_in_search_opens_by_its_entity_key_not_its_bare_id', () => {
  const opened: unknown[] = [];
  const rows: Record<string, unknown[]> = {
    session: [{ id: 's1', title: 'Fix the login', state: 'done', updated_at: 0, summary: { cwd: '/x' } }],
    project: [],
    setting: [],
  };
  const ctx = {
    client: {
      home: () => 'peer1',
      entities: { list: (k: string) => () => rows[k] ?? [], row: () => () => undefined, keys: (k: string) => () => (k === 'session' ? ['peer1:s1'] : []) },
      scope: { timers: { now: () => 0 } },
    },
    t: (k: string) => k,
    navigate: (n: unknown) => opened.push(n),
  } as never;
  const items = taskItems(ctx);
  expect(items.map((i) => i.id)).toEqual(['task:peer1:s1']);
  items[0]?.run?.();
  expect(opened).toEqual([{ t: 'openTask', id: 'peer1:s1' }]);
});
