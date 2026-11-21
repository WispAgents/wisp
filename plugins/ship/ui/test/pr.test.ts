// Whether Ship it shows (src/model/pr.ts shippable, v1 ship.js shippable), on the row fields the server
// now sends: `branch` (wisp.worktrees), `pr` and `toShip` (wisp.ship's slot, rechecked at each turn's end).
import { expect, test } from 'vitest';
import { shippable } from '../src/model/pr.ts';
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
