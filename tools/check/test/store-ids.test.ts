import assert from 'node:assert/strict';
import test from 'node:test';
import { storeIds } from '../rules/store-ids.ts';
import { fixture, messages, run } from './helpers.ts';

test("store-ids: a defineStore id outside the plugin's client namespace fails (wisp.goal → goal.), its own, computed ids and shell.* pass", async () => {
  const root = fixture('store-ids');
  assert.deepEqual(await run(storeIds, root), ['packages/shell/src/index.ts:2 store-ids', 'plugins/goal/ui/src/store.ts:1 store-ids']);
  assert.match((await messages(storeIds, root)).join('\n'), /name it "goal\.launcher"/);
});
