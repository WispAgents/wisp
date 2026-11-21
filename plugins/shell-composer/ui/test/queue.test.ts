// The queue's model (src/model/queue.ts, v1 queued.js): what each row says, and the moves the list makes.
import { expect, test } from 'vitest';
import type { EntityKey, EntityReader, LocalOp } from '@wisp/client';
import { edit, queueRowOps, remove, reorder, tagOf, taskQueue, type QueueItemRow } from '../src/model/queue.ts';
import { queueCancel, queueOrder } from '../src/commands.ts';
import type { QueueItem, Row } from '../src/types.ts';

const q: readonly Row<QueueItem>[] = [
  { id: 'a', text: 'one', at: 1 },
  { id: 'b', text: 'two', at: 2 },
  { id: 'c', text: 'three', at: 3 },
];

test('the first waits for the run, or after a Stop for you; the rest are numbered_v1_queue_held_after_stop', () => {
  expect(tagOf(0, false, 'Claude Code')).toEqual({ key: 'shell-composer.queue.next', vars: { name: 'Claude Code' } });
  expect(tagOf(0, true, 'Claude Code')).toEqual({ key: 'shell-composer.queue.held', vars: {} });
  expect(tagOf(2, true, 'Claude Code')).toEqual({ key: 'shell-composer.queue.nth', vars: { n: 3 } });
});

test('reorder, cancel and edit move or change only what was asked_v1_queue_moves', () => {
  expect(reorder(q, ['b', 'a', 'c']).map((x) => x.id)).toEqual(['b', 'a', 'c']);
  // an id the client doesn't know keeps its place at the end, nothing is lost
  expect(reorder(q, ['c']).map((x) => x.id)).toEqual(['c', 'a', 'b']);
  expect(remove(q, 'b').map((x) => x.id)).toEqual(['a', 'c']);
  expect(edit(q, 'a', '  first  ')[0]?.text).toBe('first');
  expect(edit(q, 'a', '   ')).toEqual(q); // emptying isn't cancelling
});

// A real peer (wisp.runtime rows.rs) strips `queue` from the task's row and syncs each follow-up as its own `queue_item`
// row on the task's topic. The box used to read only `body.queue`, so on `wisp serve` it showed nothing queued.
test('a_real_peers_queue_is_read_from_its_queue_item_rows_in_pos_order_v1_c_queued', () => {
  const rows: QueueItemRow[] = [
    { id: 'b', session_id: 's1', pos: 1, body: { id: 'b', text: 'two', at: 2 } },
    { id: 'x', session_id: 's2', pos: 0, body: { id: 'x', text: 'another task', at: 1 } },
    { id: 'a', session_id: 's1', pos: 0, body: { id: 'a', text: 'one', at: 1 } },
  ];
  expect(taskQueue({ body: {} }, rows, 's1').map((x) => x.id)).toEqual(['a', 'b']);
  // the dev mock and a test's flat row carry the queue themselves
  expect(taskQueue({ body: { queue: q } }, rows, 's1')).toBe(q);
  expect(taskQueue(undefined, [], 's1')).toEqual([]);
});

// Reorder, Send now, Combine and × answer at once on a real peer: the patch writes the rows save_queue would write.
test('a_queue_change_patches_the_task_queue_item_rows_at_once_on_a_real_peer_v1_queue_moves', () => {
  const key = 'local:s1' as EntityKey;
  const was = q;
  const items: QueueItemRow[] = was.map((x, pos) => ({ id: x.id, session_id: 's1', pos, body: x }));
  const base = {
    peek: (kind: string) => (kind === 'session' ? { id: 's1', state: 'running', body: {} } : undefined),
    list: (kind: string) => () => (kind === 'queue_item' ? items : []),
  } as unknown as EntityReader;
  const run = <A,>(c: { optimistic?: unknown }, a: A): readonly LocalOp[] => {
    if (typeof c.optimistic !== 'function') throw new Error('no optimistic patch');
    return (c.optimistic as (b: EntityReader, a: A) => readonly LocalOp[])(base, a);
  };
  const ops = run(queueOrder, { session: key, order: ['c', 'a', 'b'] });
  expect(ops.filter((o) => o.e === 'queue_item').map((o) => (o.op === 'upsert' ? [o.id, (o.v as QueueItemRow).pos] : [o.id, o.op]))).toEqual([
    ['local:c', 0],
    ['local:a', 1],
    ['local:b', 2],
  ]);
  const gone = run(queueCancel, { session: key, item: 'b' });
  expect(gone.find((o) => o.op === 'del')?.id).toBe('local:b');
  expect(gone.some((o) => o.e === 'session')).toBe(false); // two still wait: the hold stays
  expect(queueRowOps(key, was, [])).toHaveLength(3);
});
