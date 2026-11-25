// Owner's desktop test 2026-11-17 #12: Save for later toasted, and nothing moved. The list wrote it with runtime.setExt
// (ext["forLater"]), but the server's summary reads For later from ext["wisp.runtime"].forLater, which only
// runtime.forLater writes (plugins/runtime/host/src/ops.rs for_later, core summary.rs): the row's optimistic For later
// went back as the server's row landed. And on a server row (its view under `summary`) the toggle read the raw row, so
// a saved task never read as saved: Unsave saved it again.
import { expect, test } from 'vitest';
import type { EntityKey, PluginClient } from '@wisp/client';
import type { Toaster } from '@wisp/ui';
import { toggleDone, toggleForLater } from '../src/actions.ts';
import { flatSession } from '../src/model/wire.ts';
import { isForLater } from '../src/model/sections.ts';
import type { SessionRow } from '../src/types.ts';

const key = 'local:01a1db45c9977e1a8ae0e6b6d3c576cb' as EntityKey;
const id = '01a1db45c9977e1a8ae0e6b6d3c576cb';

interface Sent {
  readonly cmd: string;
  readonly wire: unknown;
  readonly ops: unknown;
}

/** A server-shaped row: the hot columns on top, the list's view under `summary` (crates' session row). */
const serverRow = (forLater?: { at: number; by: 'you' }): SessionRow =>
  ({ id, title: 'T', state: 'review', updated_at: 5, body: {}, summary: { status: 'review', updatedAt: 5, ...(forLater && { forLater }) } }) as unknown as SessionRow;

function ctx(row: SessionRow) {
  const sent: Sent[] = [];
  const toasts: string[] = [];
  const client = {
    entities: { peek: () => row },
    run: (cmd: { id: string; wire?: (a: unknown) => unknown; optimistic?: unknown }, args: unknown) => {
      const opt = cmd.optimistic;
      const base = { peek: () => row };
      sent.push({ cmd: cmd.id, wire: cmd.wire ? cmd.wire(args) : args, ops: typeof opt === 'function' ? (opt as (b: unknown, a: unknown) => unknown)(base, args) : opt });
      return Promise.resolve();
    },
  } as unknown as PluginClient;
  const toast = { show: (m: string) => void toasts.push(m), success: (m: string) => void toasts.push(m) } as unknown as Toaster;
  return { c: { client, t: (k: string) => k, toast, now: () => 1000 }, sent, toasts };
}

test('save_for_later_sends_runtime_forLater_the_command_whose_field_the_server_summary_reads', () => {
  const { c, sent } = ctx(serverRow());
  toggleForLater(c, key);
  expect(sent.map((s) => [s.cmd, s.wire])).toEqual([['runtime.forLater', { id, forLater: true, at: 1000 }]]);
});

test('save_for_later_puts_the_row_in_for_later_at_once_on_a_server_row', () => {
  const { c, sent } = ctx(serverRow());
  toggleForLater(c, key);
  const [{ ops } = { ops: undefined }] = sent;
  // the optimistic patch writes the summary's field, where the list (flatSession) and the server both keep it
  expect(ops).toEqual([{ e: 'session', id: key, op: 'patch', ops: [{ op: 'add', path: '/summary/forLater', value: { at: 1000, by: 'you' } }] }]);
  const after = flatSession({ ...serverRow(), summary: { status: 'review', updatedAt: 5, forLater: { at: 1000, by: 'you' } } } as unknown as SessionRow);
  expect(isForLater(after)).toBe(true);
});

test('a_saved_server_row_reads_as_saved_so_the_toggle_unsaves_it', () => {
  const { c, sent, toasts } = ctx(serverRow({ at: 900, by: 'you' }));
  toggleForLater(c, key);
  expect(sent.map((s) => [s.cmd, s.wire])).toEqual([['runtime.forLater', { id, forLater: false }]]);
  expect(sent[0]?.ops).toEqual([{ e: 'session', id: key, op: 'patch', ops: [{ op: 'remove', path: '/summary/forLater' }] }]);
  expect(toasts).toEqual(['shell-list.toast.unsaved']);
});

// design/for-later.md: Mark done takes a saved task out of For later, into Done. setState's optimistic patch removed
// `/forLater`, which a server row doesn't have (its mark is under `summary`), and the server kept the mark: a saved task
// marked done stayed in For later.
test('mark_done_on_a_saved_server_row_takes_it_out_of_for_later_at_once', () => {
  const { c, sent } = ctx(serverRow({ at: 900, by: 'you' }));
  (c.client as unknown as { navigate: () => void }).navigate = () => {};
  toggleDone(c, key);
  expect(sent.map((s) => [s.cmd, s.wire])).toEqual([['runtime.setState', { id, state: 'done' }]]);
  expect(sent[0]?.ops).toEqual([{ e: 'session', id: key, op: 'patch', ops: [{ op: 'replace', path: '/state', value: 'done' }, { op: 'remove', path: '/summary/forLater' }] }]);
});

test('undo_of_mark_done_reopens_then_saves_it_again_with_its_old_time', async () => {
  const toasts: { undo?: () => void }[] = [];
  const { c, sent } = ctx(serverRow({ at: 900, by: 'you' }));
  (c.client as unknown as { navigate: () => void }).navigate = () => {};
  const push = (_m: string, o?: { action?: { onClick: () => void } }): void => void toasts.push({ ...(o?.action && { undo: o.action.onClick }) });
  const cc = { ...c, toast: { show: push, success: push } as unknown as Toaster };
  toggleDone(cc, key);
  toasts[0]?.undo?.();
  await Promise.resolve();
  await Promise.resolve();
  expect(sent.slice(1).map((s) => [s.cmd, s.wire])).toEqual([
    ['runtime.setState', { id, state: 'review' }],
    ['runtime.forLater', { id, forLater: true, at: 900 }],
  ]);
});
