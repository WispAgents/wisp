// Whether Ship it shows (src/model/pr.ts shippable, v1 ship.js shippable), on the row fields the server
// now sends: `branch` (wisp.worktrees), `pr` and `toShip` (wisp.ship's slot, rechecked at each turn's end).
import { expect, test } from 'vitest';
import { prChipLabel, prChipTitle, prsOf, shippable } from '../src/model/pr.ts';
import { prStripSub, prStripTitle } from '../src/model/strip.ts';
import { dialogTitle } from '../src/model/ship.ts';
import type { ShipSession } from '../src/types.ts';
import { shipRow } from '../src/row.ts';
import { reviewState } from '../src/model/review.ts';
import { review } from '../src/commands.ts';
import { t } from '../src/i18n.ts';
import '../src/i18n/lazy.ts';

const row = (over: Partial<ShipSession> = {}): ShipSession => ({ id: 'p:t1', agent: 'claude', state: 'idle', branch: 'wisp/fix-login', ...over });
const inPlace = (over: Partial<ShipSession>): ShipSession => {
  const { branch: _, ...rest } = row(over);
  return rest;
};
const zero = { dirty: 0, ahead: 0, unpushed: 0 };

test('nothing to ship when the task finishes: Ship it does not show_F_ship_2', () => {
  expect(shippable(row({ toShip: zero }))).toBe(false);
  expect(shippable(row({ toShip: { dirty: 1, ahead: 0, unpushed: 0 } }))).toBe(true);
  expect(shippable(row({ toShip: { dirty: 1, ahead: 0, unpushed: 0 }, state: 'running' })), 'not while it runs').toBe(false);
  expect(shippable(inPlace({ toShip: { dirty: 1, ahead: 0, unpushed: 0 } })), 'a task in place has no Ship it').toBe(false);
  expect(shippable(row({ toShip: { dirty: 0, ahead: 2, unpushed: 0 }, pr: { number: 3, url: 'u', state: 'OPEN' } })), 'its open PR has every commit').toBe(false);
});

test('after its PR merged, more changes bring Ship it back_F_ship_7', () => {
  const merged = { number: 4, url: 'u', state: 'MERGED' as const };
  expect(shippable(row({ pr: merged, toShip: { dirty: 0, ahead: 3, unpushed: 0 } })), 'the merged commits are not news').toBe(false);
  expect(shippable(row({ pr: merged, toShip: { dirty: 2, ahead: 3, unpushed: 0 } }))).toBe(true);
  expect(shippable(row({ pr: merged, toShip: { dirty: 0, ahead: 4, unpushed: 1 } }))).toBe(true);
});

test('the Ship it dialog is titled by its guess, also when opened from the task page with only its key', () => {
  const t = (key: string, p: Readonly<Record<string, string | number>> = {}): string => `${key}${p['n'] ?? ''}`;
  // parseShipParams' guess for `{ key }` alone
  expect(dialogTitle(t, { repos: 1, update: false, branches: [] })).toBe('wisp.ship.dialog.ship');
  expect(dialogTitle(t, { repos: 1, update: true, pr: 412, branches: ['b'] })).toBe('wisp.ship.dialog.pushOne412');
});

test('the synced store row reads flat, keyed by its entity key; a flat row passes through', async () => {
  const store = { id: '01a1', state: 'review', agent: 'claude', title: 'x', summary: { branch: 'wisp/a', toShip: { dirty: 1, ahead: 0, unpushed: 0 }, state: 'review', pr: null } };
  const r = shipRow(store, 'local:01a1');
  expect(r?.id).toBe('local:01a1');
  expect(r?.branch).toBe('wisp/a');
  expect(shippable(r)).toBe(true);
  expect(shipRow({ id: 'p:t1', agent: 'claude', branch: 'b' })?.branch).toBe('b');
  expect(shipRow(undefined, 'k')).toBeUndefined();
});

// Audit E5: a failed review read "Not reviewed", as one never run; Review again showed no progress. A failed review says
// so (its why in the title), and the click's patch shows Reviewing at once.
test('a failed review says so and Review again shows Reviewing at once_audit_E5', () => {
  const ctx = { reviewOn: true, agentName: () => 'Codex' };
  const failed = { id: 'local:s1', pr: { state: 'OPEN', number: 2, head: 'abc' }, review: { status: 'failed', current: true, head: 'abc', why: 'Codex timed out.', findings: [] } };
  const v = reviewState(t, failed as never, ctx);
  expect(v?.label).toBe('Review failed');
  expect(v?.title).toContain('Codex timed out');
  const row = { id: 's1', summary: { pr: failed.pr, review: failed.review } };
  const ops = review.optimistic === 'none' || typeof review.optimistic !== 'function' ? [] : review.optimistic({ peek: () => row } as never, { key: 'local:s1' as never });
  const next = (ops[0] as { ops: { value: unknown }[] }).ops[0]?.value;
  expect(reviewState(t, { ...failed, review: next } as never, ctx)?.label).toBe('Reviewing');
});

test('prsOf deduplicates a PR present in both own and attached lists', () => {
  const own = { number: 840, url: 'https://github.com/o/wisp/pull/840', state: 'MERGED' as const };
  const other = { number: 841, url: 'https://github.com/o/wisp/pull/841', state: 'OPEN' as const };
  const s = row({ project: 'wisp', pr: own, prs: [{ repo: 'wisp', pr: own }, { repo: 'wisp', pr: other }] });
  expect(prsOf(s).map((x) => x.pr.number)).toEqual([840, 841]);
});

test('prChipLabel and prChipTitle omit repo prefix cleanly when repo is missing', () => {
  const pr = { number: 840, url: 'https://github.com/o/wisp/pull/840', state: 'MERGED' as const };
  expect(prChipLabel({ repo: 'wisp', pr })).toBe('wisp #840');
  expect(prChipLabel({ repo: undefined, pr })).toBe('#840');
  expect(prChipTitle(t, { repo: 'wisp', pr })).toBe('wisp: PR #840: Merged');
  expect(prChipTitle(t, { repo: '', pr })).toBe('PR #840: Merged');
});

test('prStripTitle and prStripSub handle single PR and every multi-PR combination', () => {
  const merged = (n: number) => ({ repo: 'wisp', pr: { number: n, url: `https://github.com/o/wisp/pull/${n}`, state: 'MERGED' as const } });
  const closed = (n: number) => ({ repo: 'wisp', pr: { number: n, url: `https://github.com/o/wisp/pull/${n}`, state: 'CLOSED' as const } });
  const open = (n: number, over: Partial<{ conflict: boolean; checks: { pass: number; fail: number; pending: number } }> = {}) => ({
    repo: 'api',
    pr: { number: n, url: `https://github.com/o/api/pull/${n}`, state: 'OPEN' as const, ...over },
  });

  expect(prStripTitle(t, [merged(840)])).toBe('PR #840');
  expect(prStripSub(t, [merged(840)])).toBe('Merged');

  expect(prStripTitle(t, [merged(839), merged(840)])).toBe('2 PRs');
  expect(prStripSub(t, [merged(839), merged(840)])).toBe('All merged');
  expect(prStripSub(t, [closed(1), closed(2)])).toBe('All closed');

  const mixed = [
    open(1, { conflict: true }),
    open(2, { checks: { fail: 1, pending: 0, pass: 2 } }),
    open(3, { checks: { fail: 0, pending: 1, pass: 2 } }),
    open(4, { checks: { fail: 0, pending: 0, pass: 3 } }),
    merged(5),
    closed(6),
  ];
  expect(prStripTitle(t, mixed)).toBe('6 PRs');
  expect(prStripSub(t, mixed)).toBe('1 conflict · 1 failing · 1 running · 1 open · 1 merged · 1 closed');
});
