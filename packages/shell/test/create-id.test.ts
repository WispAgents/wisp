// A task made from the client (New task, a run of a compare, Try it two ways, Rerun, Fork) opens its row and page at
// once under an id the client made; the peer must make the task under that same id, or the page waits on a row that
// never comes (the owner, 2026-11-14: a blank page after Send). So each create sends the bare id of the row it shows,
// in the peer's own shape (32 hex, no peer prefix, no dashes).
import { expect, test } from 'vitest';
import { entityKey, newRowId, type PeerId } from '@wisp/client';
import { create, fork, trial } from '../src/frame/commands.ts';
import { optimisticOf, peekReader, wireOf } from './command.ts';

const HEX32 = /^[0-9a-f]{32}$/;

test('a new row id is the peer own shape: 32 lowercase hex, time ordered_create_id_shape', () => {
  const a = newRowId(1_000);
  const b = newRowId(2_000);
  expect(a).toMatch(HEX32);
  expect(a < b).toBe(true);
});

test('New task sends the bare id of the row and page it opened, so the peer makes the task under it_create_sends_shown_id', () => {
  const key = entityKey('local' as PeerId, newRowId());
  const args = { peer: 'local', id: key, prompt: 'fix add', agent: 'claude', mode: 'auto', at: 1 };
  const sent = wireOf(create)(args) as { id: string };
  const shown = optimisticOf(create)(peekReader(undefined), args)[0];
  expect(sent.id).toMatch(HEX32);
  expect(shown?.id).toBe(`local:${sent.id}`);
});

test('Fork sends the bare id its new row is shown under_fork_sends_shown_id', () => {
  const newId = entityKey('local' as PeerId, newRowId());
  const sent = wireOf(fork)({ id: 'local:0123', newId, at: 1 }) as { id: string; newId: string };
  expect(sent.id).toBe('0123');
  expect(`local:${sent.newId}`).toBe(newId);
  expect(sent.newId).toMatch(HEX32);
});

test('Try it two ways sends both runs bare ids, A then B_trial_sends_shown_ids', () => {
  const ids = [entityKey('local' as PeerId, newRowId()), entityKey('local' as PeerId, newRowId())] as const;
  const sent = wireOf(trial)({ peer: 'local', ids, group: 'g', vary: { skills: 'off' }, task: { prompt: 'p', agent: 'claude', mode: 'auto', at: 1 } }) as { ids: string[] };
  expect(sent.ids.map((i) => `local:${i}`)).toEqual([...ids]);
});
