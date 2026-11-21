import assert from 'node:assert/strict';
import test from 'node:test';
import { dynamicImportInTest } from '../rules/dynamic-import-in-test.ts';
import { fixture, messages, run } from './helpers.ts';

test('dynamic-import-in-test: import() and vi.importActual inside test/it/each/hook bodies fail; top-level, vi.mock/doMock factories, allow comments with a reason and non-test files pass', async () => {
  const root = fixture('dynamic-import-in-test');
  assert.deepEqual(await run(dynamicImportInTest, root), [
    'packages/ui/test/bad.test.ts:11 dynamic-import-in-test', // test.each(...)(…, function () {})
    'packages/ui/test/bad.test.ts:15 dynamic-import-in-test', // beforeEach
    'packages/ui/test/bad.test.ts:18 dynamic-import-in-test', // a helper declared inside beforeAll
    'packages/ui/test/bad.test.ts:22 dynamic-import-in-test', // vi.importActual
    'packages/ui/test/bad.test.ts:26 dynamic-import-in-test', // the allow comment has no reason
    'packages/ui/test/bad.test.ts:3 dynamic-import-in-test', // test
    'packages/ui/test/bad.test.ts:8 dynamic-import-in-test', // it.only inside describe
  ]);
  const all = (await messages(dynamicImportInTest, root)).join('\n');
  assert.match(all, /import it at the top of the file/);
  assert.match(all, /vi\.importActual\("\.\.\/src\/index\.ts"\)/);
  assert.match(all, /check:allow dynamic-import-in-test <reason>/);
});
