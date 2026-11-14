// The list's writes name the task by its entity key (`local:<id>`); the peer knows it by its bare id. Each command
// sends the bare id, or the peer answers "no task local:…" (found on a real server, 2026-11-14: opening a task from
// the list toasted it, once the welcome stopped refusing Mark read).
import { expect, test } from 'vitest';
import type { EntityKey } from '@wisp/client';
import { archive, markRead, remove, rename, setExt, setState } from '../src/commands.ts';

const key = 'local:01a1db45c9977e1a8ae0e6b6d3c576cb' as EntityKey;
const id = '01a1db45c9977e1a8ae0e6b6d3c576cb';
const sent = (wire: ((a: never) => unknown) | undefined, a: unknown): unknown => (wire ? wire(a as never) : a);

test('every list write sends the peer the bare task id, never the entity key_list_writes_send_bare_ids', () => {
  expect(sent(markRead.wire, { id: key, unread: false })).toEqual({ id, unread: false });
  expect(sent(rename.wire, { id: key, title: 'T' })).toEqual({ id, title: 'T' });
  expect(sent(archive.wire, { id: key, archived: true })).toEqual({ id, archived: true });
  expect(sent(setState.wire, { id: key, state: 'done' })).toEqual({ id, state: 'done' });
  expect(sent(remove.wire, { id: key })).toEqual({ id });
  expect(sent(setExt.wire, { id: key, plugin: 'pinned', value: true })).toEqual({ id, plugin: 'pinned', value: true });
});
