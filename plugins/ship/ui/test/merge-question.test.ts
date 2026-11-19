// QA r14 #5: the Merge PR confirm reads as one sentence: "Squash-merge PR #806?" alone, or with what wisp does after it
// as its own sentence; never "?." (the after-part was empty and the template's full stop hung on the question mark).
import { expect, test } from 'vitest';
import { t } from '../src/i18n.ts';
import { mergePlan } from '../src/model/merge.ts';
import type { ShipSession } from '../src/types.ts';

const ctx = { reviewOn: false, agentName: () => undefined };
const pr = { number: 806, url: 'https://github.com/o/r/pull/806', state: 'OPEN' as const };

test('a_pr_its_agent_attached_asks_only_the_question', () => {
  const s: ShipSession = { id: 's1', agent: 'claude', prs: [{ repo: 'o/r', pr }] };
  expect(mergePlan(t, s, ctx)?.question).toBe('Squash-merge PR #806?');
});

test('a_pr_ship_it_opened_says_what_follows_as_its_own_sentence', () => {
  const s: ShipSession = { id: 's1', agent: 'claude', project: 'o/r', pr };
  const q = mergePlan(t, s, ctx)?.question ?? '';
  expect(q).toBe('Squash-merge PR #806? Then wisp fast-forwards your local base branch and removes the worktree, unless it has uncommitted work.');
  expect(q).not.toMatch(/\?\./);
});

// Audit B #19: the confirm said it "removes the worktree" when the merge would keep it (uncommitted work), and only the
// toast after told the truth.
test('a_worktree_with_uncommitted_work_is_said_to_stay_before_the_merge', () => {
  const s: ShipSession = { id: 's1', agent: 'claude', project: 'o/r', pr, toShip: { dirty: 1, ahead: 0, unpushed: 0 } };
  expect(mergePlan(t, s, ctx)?.question).toBe('Squash-merge PR #806? Then wisp fast-forwards your local base branch, and keeps the worktree: it has 1 uncommitted change.');
  const clean: ShipSession = { ...s, toShip: { dirty: 0, ahead: 0, unpushed: 0 } };
  expect(mergePlan(t, clean, ctx)?.question).toContain('and removes the worktree, unless it has uncommitted work.');
});

// Audit C (B #19 still open): what the task has to ship is rechecked after a turn, not when a file changes outside one, so
// `toShip` said 0 uncommitted while the worktree held work; the merge kept it and the confirm had said "removes the
// worktree". The merge decides when it runs: the words say what it does then, whatever `toShip` said before.
test('the_confirm_never_promises_a_removal_the_merge_may_not_do_audit_c_b19', () => {
  const s: ShipSession = { id: 's1', agent: 'claude', project: 'o/r', pr };
  for (const toShip of [undefined, { dirty: 0, ahead: 0, unpushed: 0 }]) {
    const q = mergePlan(t, { ...s, ...(toShip && { toShip }) }, ctx)?.question ?? '';
    expect(q).not.toMatch(/removes the worktree\.$/);
    expect(q).toContain('unless it has uncommitted work');
  }
});
