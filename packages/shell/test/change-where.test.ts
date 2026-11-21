// Audit F setup F2: a new task's page crashed right after it started ("This part of wisp hit an error · Unexpected end
// of JSON input"). The head's changes read had '' as its source while the task had no worktree yet; Solid's
// createResource fetches for '' (it skips only undefined, null and false), so JSON.parse('') threw into the resource.
import { expect, test } from 'vitest';
import { createResource, createRoot } from 'solid-js';
import { changeWhere, whereOf } from '../src/frame/change-where.ts';

test('a_task_with_no_worktree_yet_has_no_changes_read_audit_f_setup_f2', async () => {
  const fresh = { summary: {}, body: { cwd: '/repo' } };
  expect(changeWhere(fresh, undefined)).toBeUndefined();
  expect(changeWhere(undefined, undefined)).toBeUndefined();
  // the resource never fetches for it, so it never holds a parse error
  let fetched = 0;
  await createRoot(async (dispose) => {
    const [r] = createResource(
      () => changeWhere(fresh, undefined),
      () => (fetched++, 1),
    );
    await Promise.resolve();
    expect(r.error).toBeUndefined();
    dispose();
  });
  expect(fetched).toBe(0);
  // once the worktree is there: its folder and base, read back from the key
  const wt = { summary: { toShip: { dirty: 1 } }, body: { ext: { 'wisp.worktrees': { path: '/w/t1', baseRef: 'origin/main' } } } };
  const key = changeWhere(wt, { id: 't1', status: 'done' });
  expect(key && whereOf(key)).toEqual({ path: '/w/t1', base: 'origin/main' });
  expect(changeWhere({ summary: { worktree: { cwd: '/w/t2' } } }, undefined)).toBeDefined();
  // a key that isn't one is no read, never a throw
  expect(whereOf('')).toBeUndefined();
  expect(whereOf('[1]')).toBeUndefined();
});
