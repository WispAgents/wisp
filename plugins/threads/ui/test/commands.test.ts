// A rewrite shows at once, as the server will store it (notes.rs update): the section's row (a new one when it was
// never written), its rev one up, by you; the Notes' rev one up; the same text again changes nothing.
import { expect, test } from 'vitest';
import type { EntityKey, EntityReader } from '@wisp/client';
import { fresh, notesUpdate, rewriteOps } from '../src/commands.ts';
import type { MetaRow, NoteRow } from '../src/types.ts';

const KEY = 'local:s0002' as EntityKey;
function reader(rows: Record<string, unknown>): EntityReader {
  return { peek: (kind: string, key: string) => rows[`${kind}/${key}`] } as unknown as EntityReader;
}
const note: NoteRow = { id: 's0002|goal', task_id: 's0002', section: 'goal', text: 'Old goal', rev: 2, by: 'agent', at: 1 };
const meta: MetaRow = { id: 's0002', rev: 7, seen: 7 };

test('a rewrite upserts the section as you wrote it, its rev one up, and moves the Notes rev', () => {
  const ops = rewriteOps(reader({ 'threads_notes/local:s0002|goal': note, 'threads_meta/local:s0002': meta }), { key: KEY, section: 'goal', text: '  New goal  ' }, 50);
  expect(ops).toEqual([
    { e: 'threads_notes', id: 'local:s0002|goal', op: 'upsert', v: { id: 's0002|goal', task_id: 's0002', section: 'goal', text: 'New goal', rev: 3, by: 'you', at: 50 } },
    { e: 'threads_meta', id: 'local:s0002', op: 'patch', ops: [{ op: 'replace', path: '/rev', value: 8 }] },
  ]);
});

test('a section never written gets its first row; a list is kept clean as the server keeps it', () => {
  const ops = rewriteOps(reader({}), { key: KEY, section: 'constraints', text: '- No Redis\n- no redis.\n2. Keep the API' }, 9);
  expect(ops).toEqual([{ e: 'threads_notes', id: 'local:s0002|constraints', op: 'upsert', v: { id: 's0002|constraints', task_id: 's0002', section: 'constraints', text: 'No Redis\nKeep the API', rev: 1, by: 'you', at: 9 } }]);
});

test('the same text again shows no change (and the server writes no rev)', () => {
  expect(rewriteOps(reader({ 'threads_notes/local:s0002|goal': note }), { key: KEY, section: 'goal', text: 'Old goal\n' }, 5)).toEqual([]);
});

test('the wire takes the bare id the api takes; the task’s peer runs it; Fresh thread needs a wisp that has it', () => {
  expect(notesUpdate.wire?.({ key: KEY, section: 'goal', text: 'x' })).toEqual({ id: 's0002', section: 'goal', text: 'x', by: 'you' });
  expect(notesUpdate.owner({ key: KEY, section: 'goal', text: 'x' })).toBe(KEY);
  expect(fresh.wire?.({ key: KEY, brief: 'B' })).toEqual({ id: 's0002', brief: 'B' });
  expect(fresh.feature).toEqual({ plugin: 'wisp.threads', command: 'threads.fresh', label: 'Fresh thread' });
});
