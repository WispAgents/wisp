// perf-ui-r2 #1: ⌘K looked up each of ~4,500 tasks' keys with a scan of every key (O(n²), an 800 ms open at 4×). One
// bare id → key index per keys list, shared by every caller holding that list.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { keyOfId } from '../src/ids.ts';

/** `keys` as a list that counts how many of its items are read. */
const counted = (keys: string[]): { list: readonly string[]; reads: () => number } => {
  let n = 0;
  const list = new Proxy(keys, {
    get(t, k, r) {
      if (typeof k === 'string' && /^\d+$/.test(k)) n++;
      return Reflect.get(t, k, r);
    },
  });
  return { list, reads: () => n };
};

test('looking_up_every_id_of_5000_keys_reads_the_list_once_not_once_per_id', () => {
  const ids = Array.from({ length: 5000 }, (_, i) => `t${i}`);
  const { list, reads } = counted(ids.map((id, i) => `${i % 2 ? 'box' : 'laptop'}:${id}`));
  for (const id of ids) assert.equal(keyOfId(list, id)?.endsWith(`:${id}`), true);
  assert.equal(reads(), 5000, 'one pass to build the index, then lookups');
  assert.equal(keyOfId(list, 'nope'), undefined);
  assert.equal(reads(), 5000, 'a miss is a lookup too');
});

test('the_first_key_in_order_wins_and_a_new_list_is_indexed_again', () => {
  const a = ['box:t1', 'laptop:t1', 'laptop:t2'];
  assert.equal(keyOfId(a, 't1'), 'box:t1', 'as find() did: the first in order');
  assert.equal(keyOfId(a, 'laptop:t1'), undefined, 'a key is no bare id');
  const b = ['laptop:t3'];
  assert.equal(keyOfId(b, 't3'), 'laptop:t3');
  assert.equal(keyOfId(b, 't1'), undefined, 'each list its own index');
});
