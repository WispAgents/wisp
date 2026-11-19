// Audit B #5: ⋯ Mark unread in an open task left for another task with no word, so the next ⋯ action (Save for later)
// was taken for the old task's. v1 had no Mark unread on the page; v2 leaves (staying would read it again), and now
// says so with Undo. Where it goes is never the task itself, and the toast names the task left.
import { expect, test } from 'vitest';
import { unreadLeave } from '../src/frame/unread-leave.ts';
import type { SessionRow } from '../src/rows.ts';

const row = (id: string, at: number, o: Partial<SessionRow> = {}): SessionRow =>
  ({ id, title: id, state: 'review', agent: 'claude', archived: false, unread: true, created_at: at, updated_at: at, summary: { status: 'done', turnCount: 1, queued: 0, preview: '', live: '', cwd: '', model: '' }, ...o }) as SessionRow;

test('mark_unread_leaves_for_the_next_task_that_needs_you_and_says_so_with_the_task_left', () => {
  const rows = [row('a', 1), row('b', 5), row('c', 3)];
  const out = unreadLeave(rows, 'local:b', 'Done one', 'local');
  expect(out.to).toEqual({ t: 'openTask', id: 'local:c' });
  expect(out.toast).toEqual({ key: 'shell.toast.unread', vars: { title: 'Done one' } });
  // nothing else needs you: home, still with the toast
  expect(unreadLeave([row('b', 5)], 'local:b', 'Done one', 'local').to).toEqual({ t: 'home' });
});
