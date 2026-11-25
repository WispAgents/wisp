// /btw and its ×: the optimistic patch lands on the task's cell (its EntityKey), not on the row's own id, which
// is bare on a real peer (putBase keys a cell as entityKey(peer, row.id)).
import { expect, test } from 'vitest';
import { btw, btwDrop } from '../src/frame/commands.ts';
import { field, optimisticOf, patchValue, peekReader } from './command.ts';

test('btw_and_btw_drop_patch_the_tasks_entity_key_not_its_bare_row_id', () => {
  const row = { id: 'abc', body: { ext: { 'wisp.runtime': {} } } };
  const asked = optimisticOf(btw)(peekReader(row), { id: 'peer1:abc', question: 'why?', tmp: 'b1', at: 5 });
  expect(asked[0]?.id).toBe('peer1:abc');
  expect(field(patchValue(asked), 'wisp.runtime', 'btw')).toEqual([{ id: 'b1', question: 'why?', at: 5, pending: true }]);
  const withOne = { id: 'abc', body: { ext: { 'wisp.runtime': { btw: [{ id: 'b1', question: 'q', at: 1 }] } } } };
  const dropped = optimisticOf(btwDrop)(peekReader(withOne), { id: 'peer1:abc', btwId: 'b1' });
  expect(dropped[0]?.id).toBe('peer1:abc');
  expect(field(patchValue(dropped), 'wisp.runtime', 'btw')).toEqual([]);
});
