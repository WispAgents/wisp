// TRY-17 #30: Pin wrote runtime.setExt (ext["pinned"]), which the server's summary never reads, so the pin went back
// as the server's row landed. It's runtime.pin now (ext["wisp.runtime"].pinned, the summary's `pinned`), with the
// row's pill at once.
import { expect, test } from 'vitest';
import type { EntityKey, PluginClient } from '@wisp/client';
import type { Toaster } from '@wisp/ui';
import { togglePin } from '../src/actions.ts';
import { flatSession } from '../src/model/wire.ts';
import { pillsOf } from '../src/model/row.ts';
import type { SessionRow } from '../src/types.ts';

const key = 'local:01a1db45c9977e1a8ae0e6b6d3c576cb' as EntityKey;
const id = '01a1db45c9977e1a8ae0e6b6d3c576cb';
const serverRow = (pinned?: boolean): SessionRow =>
  ({ id, title: 'T', state: 'review', updated_at: 5, body: {}, summary: { status: 'done', updatedAt: 5, ...(pinned && { pinned }) } }) as unknown as SessionRow;

function ctx(row: SessionRow) {
  const sent: { cmd: string; wire: unknown; ops: unknown }[] = [];
  const client = {
    entities: { peek: () => row },
    run: (cmd: { id: string; wire?: (a: unknown) => unknown; optimistic?: unknown }, args: unknown) => {
      const opt = cmd.optimistic;
      sent.push({ cmd: cmd.id, wire: cmd.wire ? cmd.wire(args) : args, ops: typeof opt === 'function' ? (opt as (b: unknown, a: unknown) => unknown)({ peek: () => row }, args) : opt });
      return Promise.resolve();
    },
  } as unknown as PluginClient;
  const toast = { show: () => {}, success: () => {} } as unknown as Toaster;
  return { c: { client, t: (k: string) => k, toast, now: () => 1000 }, sent };
}

test('pin_sends_runtime_pin_the_command_whose_field_the_server_summary_reads', () => {
  const { c, sent } = ctx(serverRow());
  togglePin(c, key);
  expect(sent.map((s) => [s.cmd, s.wire])).toEqual([['runtime.pin', { id, pinned: true }]]);
  // the summary's field at once, where the server's row keeps it too: the pill shows and stays
  expect(sent[0]?.ops).toEqual([{ e: 'session', id: key, op: 'patch', ops: [{ op: 'add', path: '/summary/pinned', value: true }] }]);
  const after = flatSession({ ...serverRow(), summary: { status: 'done', updatedAt: 5, pinned: true } } as unknown as SessionRow);
  expect(pillsOf(after).some((p) => p.id === 'pinned')).toBe(true);
});

test('a_pinned_server_row_reads_as_pinned_so_the_toggle_unpins_it', () => {
  const { c, sent } = ctx(serverRow(true));
  togglePin(c, key);
  expect(sent.map((s) => [s.cmd, s.wire])).toEqual([['runtime.pin', { id, pinned: false }]]);
  expect(sent[0]?.ops).toEqual([{ e: 'session', id: key, op: 'patch', ops: [{ op: 'remove', path: '/summary/pinned' }] }]);
});
