import assert from 'node:assert/strict';
import { test } from 'node:test';
import { migrateV1 } from '../src/migrate.ts';
import { fakeBridge, memoryStorage } from './helpers.ts';

const v1 = JSON.stringify([
  { id: 'g1', kind: 'grid', grid: 'https://grid.trywisp.dev', url: 'https://mo.grid.trywisp.dev', name: 'wisp Cloud', token: 'wlt_v1_device', computers: [{ id: 'c9', name: 'Studio', url: 'https://studio--mo.grid.trywisp.dev' }] },
  { id: 'k3', target: 'me@studio' },
]);

// ADR-0065 edge case: v1's localStorage tokens move into the Keystore once, then are deleted.
test('v1_tokens_leave_web_storage', async () => {
  const ls = memoryStorage([['wisp.machines', v1], ['wisp.grid', 'grid.trywisp.dev']]);
  const bridge = fakeBridge();
  assert.equal(await migrateV1(ls, bridge), 3);
  assert.deepEqual([...bridge.tokens], [['g1', 'wlt_v1_device'], ['g1.c9', 'wlt_v1_device']], 'handed to the native side, which seals them');
  assert.ok(!JSON.stringify(await bridge.list()).includes('wlt_v1_device'), 'the saved list has none');
  assert.ok(![...ls.map.values()].some((v) => v.includes('wlt_v1_device')), 'no token left in localStorage');
  assert.equal(ls.map.has('wisp.machines'), false);
  assert.equal(ls.map.get('wisp.grid'), 'grid.trywisp.dev', 'what isn’t a secret stays');
  assert.equal(await migrateV1(ls, bridge), 0, 'once');
  assert.equal(bridge.saves.length, 3);
});

test('a copy that fails keeps v1’s list for the next start, and copies once it works', async () => {
  const ls = memoryStorage([['wisp.machines', v1]]);
  const bridge = fakeBridge();
  const save = bridge.save.bind(bridge);
  bridge.save = async () => {
    throw new Error('the Keystore is locked');
  };
  await assert.rejects(migrateV1(ls, bridge), /locked/);
  assert.equal(ls.map.get('wisp.machines'), v1, 'nothing lost');
  bridge.save = save;
  assert.equal(await migrateV1(ls, bridge), 3);
  assert.equal(ls.map.has('wisp.machines'), false);
});

test('a list an earlier build copied but kept loses its tokens on this start', async () => {
  const ls = memoryStorage([['wisp.machines', v1], ['wisp.machines.v2', '1']]);
  const bridge = fakeBridge();
  assert.equal(await migrateV1(ls, bridge), 0, 'not copied twice (a forgotten computer stays forgotten)');
  assert.equal(ls.map.has('wisp.machines'), false);
});
