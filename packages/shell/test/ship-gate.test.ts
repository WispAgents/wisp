// Audit B #2: Ship it works on worktree tasks only (v1 routes/ship.js NO_TREE, features/ship.js shippable: no branch,
// no Ship it). A task in its project's own folder had Ship in its head anyway, and pressing it said "no such session".
import { expect, test } from 'vitest';
import { shipsFromWorktree } from '../src/frame/ship-gate.ts';

test('only_a_task_with_a_worktree_branch_offers_ship_v1_shippable', () => {
  expect(shipsFromWorktree({ branch: 'wisp/fix-login' })).toBe(true);
  // one task across repos: another folder's worktree is enough (v1 hasTrees: s.also?.some((a) => a.worktree))
  expect(shipsFromWorktree({ also: [{ project: 'api', branch: 'wisp/x' }] })).toBe(true);
  expect(shipsFromWorktree({ cwd: '/code/app' })).toBe(false);
  expect(shipsFromWorktree({ branch: '', also: [{ project: 'api' }] })).toBe(false);
  expect(shipsFromWorktree(undefined)).toBe(false);
});
