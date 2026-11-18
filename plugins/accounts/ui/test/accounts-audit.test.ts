// The owner's Settings → Agents audit (2026-11-18, /tmp/qa-keep/agents-audit.md), the pane's rules. v1 is
// web/js/features/accounts.js and agent-login.js.
import { expect, test } from 'vitest';
import type { EntityReader } from '@wisp/client';
import { addAccount, logout, updateAccount } from '../src/commands.ts';
import type { AccountRow, HarnessRow, LoginRow } from '../src/types.ts';
import { accOf, accountMenu, accountState, loginControl, mergeHarnesses, pendingHarnesses, withPending } from '../src/words.ts';
import type { MenuEntry, MenuItem } from '@wisp/ui/pure';

const item = (list: readonly MenuEntry[], v: string): MenuItem | undefined => list.find((x): x is MenuItem => 'value' in x && x.value === v);

const acc = (agent: string, id: string, more: Partial<AccountRow> = {}): AccountRow => ({
  agent,
  id: `${agent}/${id}`,
  ord: 0,
  label: id,
  email: '',
  plan: '',
  default: id === 'default',
  kind: 'personal',
  state: 'ready',
  until: null,
  why: '',
  ...more,
});

/** A store with these rows, as `optimistic` reads it. */
function base(rows: Record<string, object>): EntityReader {
  return {
    keys: (kind: string) => () => Object.keys(rows).filter((k) => k.startsWith(`${kind}|`)).map((k) => k.slice(kind.length + 1)),
    peek: (kind: string, id: string) => rows[`${kind}|${id}`],
  } as unknown as EntityReader;
}

// #1: an account row's id is its entity's (`claude/w1`); every control on it used that as the account and found no
// sign-in, signed out or removed nothing
test('an_account_rows_id_is_the_entitys_and_its_account_is_after_the_harness', () => {
  expect(accOf(acc('claude', 'w1'))).toBe('w1');
  expect(accOf(acc('claude', 'default'))).toBe('default');
  expect(accOf({ agent: 'claude', id: 'w1' })).toBe('w1'); // an optimistic row before the server's
});

test('a_new_account_appears_at_once_with_the_ids_the_server_gives_it_and_its_sign_in_under_it', () => {
  const ops = addAccount.optimistic as unknown as (b: EntityReader, a: unknown) => { e: string; id: string; v: Record<string, unknown> }[];
  const out = ops(base({}), { peer: 'p', agent: 'claude', id: 'w2', label: 'Work', kind: 'work' });
  expect(out[0]?.v['id']).toBe('claude/w2');
  expect(out[1]?.v['account']).toBe('w2');
  expect(out[1]?.v['state']).toBe('running');
});

test('move_up_swaps_the_account_with_the_one_above_it', () => {
  const rows = { 'accounts.account|p:claude/default': acc('claude', 'default', { ord: 0 }), 'accounts.account|p:claude/w1': acc('claude', 'w1', { ord: 1 }) };
  const ops = updateAccount.optimistic as unknown as (b: EntityReader, a: unknown) => { id: string; v: AccountRow }[];
  const out = ops(base(rows), { peer: 'p', agent: 'claude', account: 'w1', move: 'up' });
  expect(out.map((o) => [o.v.id, o.v.ord])).toEqual([
    ['claude/w1', 0],
    ['claude/default', 1],
  ]);
});

// a sign-in that failed offers Sign in again (v1: the row's control is Sign in, its panel the error)
test('a_failed_sign_in_offers_sign_in_again', () => {
  const failed: LoginRow = { agent: 'antigravity', account: '', state: 'failed', error: 'timed out', signedIn: null };
  expect(loginControl(failed, { account: false, out: false })).toBe('signIn');
  expect(loginControl(failed, { account: true, out: false })).toBe('signIn');
  // a cancelled one too, and an account known signed out with no sign-in row (after a restart)
  expect(loginControl({ ...failed, state: 'cancelled' }, { account: false, out: false })).toBe('signIn');
  expect(loginControl(undefined, { account: true, out: true })).toBe('signIn');
  // a signed-in account row says nothing (v1: Sign out is in its ⋯)
  expect(loginControl({ ...failed, state: 'idle', signedIn: true }, { account: true, out: false })).toBe('none');
});

// the harnesses wisp found first, the missing ones after them (v1: Copilot, not found, came last)
test('missing_harnesses_come_after_the_ones_found', () => {
  const h = (id: string, ord: number, available: boolean): HarnessRow => ({ id, name: id, ord, available, models: [], signIn: 'cli' });
  expect(mergeHarnesses([h('claude', 0, true), h('copilot', 1, false), h('grok', 2, true)], []).map((x) => x.id)).toEqual(['claude', 'grok', 'copilot']);
});

// v1's ⋯ on every account row: Rename, Make it a work / personal account, Move up, Move down, Sign out, Remove; what
// can't apply stays, disabled, so every row's menu has the same shape
test('every_account_row_has_v1s_menu_with_what_cant_apply_disabled', () => {
  const t = (k: string): string => k;
  const first = accountMenu(acc('claude', 'default', { state: 'signedOut' }), 0, 2, t);
  expect(first.map((x) => ('value' in x ? x.value : '—'))).toEqual(['rename', 'kind', 'up', 'down', 'out', '—', 'remove']);
  const off = (v: string): boolean | undefined => item(first, v)?.disabled;
  expect([off('up'), off('down'), off('out'), off('remove')]).toEqual([true, false, true, true]);
  const last = accountMenu(acc('claude', 'w1', { kind: 'work' }), 1, 2, t);
  const on = (v: string): boolean | undefined => item(last, v)?.disabled;
  expect([on('up'), on('down'), on('out'), on('remove')]).toEqual([false, true, false, false]);
  expect(item(last, 'kind')?.label).toBe('accounts.account.personal');
});

// v1: "At its limit until 9 pm", in the page's time format
test('an_account_at_its_limit_says_until_when', () => {
  const until = new Date(2026, 10, 18, 21, 0).getTime();
  const s = accountState(acc('claude', 'w1', { state: 'limit', until }), (k, v) => `${k}${v ? JSON.stringify(v) : ''}`);
  expect(s).toMatch(/^accounts\.state\.limitUntil\{"until":".+"\}$/);
  expect(accountState(acc('claude', 'w1', { state: 'signedOut' }), (k) => k)).toBe('accounts.state.signedOut');
  expect(accountState(acc('claude', 'w1'), (k) => k)).toBeUndefined();
});

// Sign out answers at once (ADR-0017) even with no sign-in row yet (GitHub's: gh was signed in outside wisp)
test('sign_out_shows_signed_out_at_once_even_before_any_sign_in_row', () => {
  const ops = logout.optimistic as unknown as (b: EntityReader, a: unknown) => { e: string; id: string; v: Record<string, unknown> }[];
  const out = ops(base({}), { peer: 'p', agent: 'gh', account: '' });
  expect(out.find((o) => o.e === 'accounts.login')?.v).toMatchObject({ agent: 'gh', account: '', state: 'idle', signedIn: false });
});

// Right after a restart (or on a first start) the pane had no rows until every harness was detected: only its lede. It
// shows a row per harness wisp knows (their plugins' names) at once, each Checking… until its answer lands.
test('before_any_answer_each_known_harness_shows_checking', () => {
  const rows = pendingHarnesses([{ id: 'claude', name: 'Claude Code', order: 0 }, { id: 'codex', name: 'Codex', order: 1 }]);
  expect(rows.map((r) => [r.id, r.name, r.checking])).toEqual([
    ['claude', 'Claude Code', true],
    ['codex', 'Codex', true],
  ]);
  expect(rows.every((r) => !r.available && r.signIn === 'none')).toBe(true);
  // the answers land one by one: those not in yet stay Checking…, the rest show (none vanishes and comes back)
  const some = withPending([{ id: 'codex', name: 'Codex', ord: 1, available: true, models: [], signIn: 'cli' }], [{ id: 'claude', name: 'Claude Code', order: 0 }, { id: 'codex', name: 'Codex', order: 1 }], true);
  expect(some.map((r) => [r.id, r.checking === true])).toEqual([
    ['claude', true],
    ['codex', false],
  ]);
  // once the pane's own rows are in, they're the list
  expect(withPending([], [{ id: 'claude', name: 'Claude Code' }], false)).toEqual([]);
});

// nothing jumps while harnesses settle: one still being checked keeps its place; only one known missing goes last
test('a_harness_still_checking_keeps_its_place', () => {
  const h = (id: string, ord: number, more: Partial<HarnessRow>): HarnessRow => ({ id, name: id, ord, available: false, models: [], signIn: 'cli', ...more });
  expect(mergeHarnesses([h('claude', 0, { checking: true }), h('copilot', 1, {}), h('agy', 2, { available: true })], []).map((x) => x.id)).toEqual(['claude', 'agy', 'copilot']);
});
