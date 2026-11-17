// B10: a held run's row says which window and how full, from the runtime's held turn ({kind, by, until, data}: runtime
// core summary.rs), never "at 0%".
import { expect, test } from 'vitest';
import type { SessionRow } from '../src/types.ts';
import { flatSession } from '../src/model/wire.ts';
import { isHeld } from '@wisp/shell/pure';
import { pillsOf, subOf, type RowContext } from '../src/model/row.ts';

const ctx = { split: false, open: false, swipes: false, here: 'This computer', agents: new Map() } as unknown as RowContext;
const held = (h: unknown): SessionRow =>
  flatSession({ id: 'a', title: 'test2 · Check-in', state: 'review', agent: 'claude', created_at: 1, updated_at: 2, body: { cwd: '/w' }, summary: { status: 'held', preview: '', held: h } } as unknown as SessionRow);

test('Ask near the limit: the window and how full it is', () => {
  const s = held({ kind: 'ask', by: 'wisp.budget', until: 9, data: { label: '5-hour', used: 0.97, why: 'x' }, turnId: 't' });
  expect(subOf(s, ctx)?.text).toEqual({ key: 'shell-list.held.window', vars: { label: '5-hour', pct: 97 } });
});

test('a stop line: it waits for the reset; a project setup; the harness watch', () => {
  const stop = held({ kind: 'plan', by: 'wisp.budget', until: 9, data: { hold: { label: 'Weekly', used: 0.95 }, why: 'x' } });
  expect(subOf(stop, ctx)?.text).toEqual({ key: 'shell-list.held.reset', vars: { label: 'weekly' } });
  const setup = held({ kind: 'setup', by: 'wisp.runtime', data: { label: 'Its setup', why: 'npm ci failed' } });
  expect(subOf(setup, ctx)?.text).toEqual({ key: 'shell-list.held.setup' });
  const watch = held({ kind: 'watch', by: 'wisp.accounts', data: { agent: 'claude', line: 'Claude Code changed' } });
  expect(subOf(watch, ctx)?.text.key).toBe('shell-list.held.harness');
});

test('no window to name: its why, never "at 0%"', () => {
  const s = held({ kind: 'other', by: 'my.plugin', data: { why: 'Waits for the deploy window' } });
  expect(subOf(s, ctx)?.text).toEqual({ key: 'shell-list.raw', vars: { text: 'Waits for the deploy window' } });
});

// TRY-17 #28: the list's Held comes from the same helper as Home's and the task header's (`@wisp/shell/pure` isHeld)
test('a_held_row_is_held_by_the_shells_one_rule', () => {
  const s = held({ kind: 'harness-changed', by: 'wisp.accounts', data: { line: 'Codex changed' } });
  expect(isHeld(s.state, { status: s.status, held: s.held })).toBe(true);
  expect(pillsOf(s).find((p) => p.id === 'status')?.label.key).toBe('shell-list.pill.held');
  // a held status with no held turn yet still says Held, as the header does
  const bare = flatSession({ id: 'b', title: 't', state: 'review', agent: 'codex', created_at: 1, updated_at: 2, body: { cwd: '/w' }, summary: { status: 'held' } } as unknown as SessionRow);
  expect(pillsOf(bare).find((p) => p.id === 'status')?.label.key).toBe('shell-list.pill.held');
});

// QA r1 #10: a task marked done while held (older data: Mark done didn't skip its turn then) sat under Done saying
// "Waiting for codex"; a done task's line never says it waits
test('a_done_task_never_says_it_waits_even_with_a_held_turn_left', () => {
  const done = flatSession({ id: 'd', title: 'test · Job', state: 'done', agent: 'codex', created_at: 1, updated_at: 2, body: { cwd: '/w' }, summary: { status: 'held', preview: '', held: { kind: 'harness-changed', by: 'wisp.accounts', data: { agent: 'codex', line: 'Codex changed' } } } } as unknown as SessionRow);
  expect(subOf(done, ctx)?.held).toBeFalsy();
  expect(subOf(done, ctx)?.text.key).not.toBe('shell-list.held.harness');
});
