// round 324: My Stuff grouped by task heads each group with its task's title, as the store holds it now: a task that
// lands after the page opened (its row on another topic), or is renamed while it shows, heads its group at once. The
// page's groups memo read the session rows straight from the store, which Solid doesn't follow.
import { expect, test } from 'vitest';
import { createMemo, createRoot } from 'solid-js';
import { signal } from '@wisp/client';
import { useTaskTitle } from '../src/items.ts';
import { groupItems } from '../src/model.ts';
import type { Item } from '../src/types.ts';

const item = (id: string, session: string): Item => ({ id, kind: 'doc', name: `${id}.md`, title: id, size: 1, at: 1_000, by: 'agent', from: { session } }) as Item;

test('grouped_by_task_a_task_that_lands_or_is_renamed_after_my_stuff_opened_heads_its_group', () => {
  const sessions = signal<readonly { id: string; title: string }[]>([]);
  const client = { entities: { list: () => sessions } };
  createRoot((dispose) => {
    const title = useTaskTitle(client as never);
    const heads = createMemo(() => groupItems([item('notes', 's1')], 'task', { t: (k) => k, now: 2_000, task: title, month: () => '' }).map((g) => g.label));
    const before = heads();
    sessions([{ id: 'laptop:s1', title: 'Fix login' }]);
    expect(heads(), 'the task landed').toContain('Fix login');
    expect(heads()).not.toEqual(before);
    sessions([{ id: 'laptop:s1', title: 'Fix the login page' }]);
    expect(heads(), 'renamed').toContain('Fix the login page');
    dispose();
  });
});
