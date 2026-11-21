import assert from 'node:assert/strict';
import test from 'node:test';
import { timerLoopInTest } from '../rules/timer-loop-in-test.ts';
import { fixture, messages, run } from './helpers.ts';

test('timer-loop-in-test: loops that only await setTimeout hops with a count fail (inline, local and imported helpers, node:timers/promises, while/do, for-of over Array(n), guards); clock deadlines, plain conditions, stepping, setImmediate and microtask loops, allow comments with a reason and src files pass', async () => {
  const root = fixture('timer-loop-in-test');
  assert.deepEqual(await run(timerLoopInTest, root), [
    'packages/ui/test/bad.test.ts:10 timer-loop-in-test', // i < 300 && !ok(): a count cap on a local helper
    'packages/ui/test/bad.test.ts:12 timer-loop-in-test', // while (n-- > 0) on an imported, renamed helper
    'packages/ui/test/bad.test.ts:13 timer-loop-in-test', // do … while (--n > 0), two hops a turn
    'packages/ui/test/bad.test.ts:18 timer-loop-in-test', // the allow comment has no reason
    'packages/ui/test/bad.test.ts:4 timer-loop-in-test', // an inline setTimeout promise, in a top-level flush
    'packages/ui/test/helpers.ts:3 timer-loop-in-test', // a helper module in test/ counts too
    'packages/ui/test/more.test.ts:11 timer-loop-in-test', // for-of over Array.from({ length })
    'packages/ui/test/more.test.ts:12 timer-loop-in-test', // for-of over new Array(n), node:timers/promises' sleep
    'packages/ui/test/more.test.ts:5 timer-loop-in-test', // while (n--)
    'packages/ui/test/more.test.ts:6 timer-loop-in-test', // i !== 30
    'packages/ui/test/more.test.ts:7 timer-loop-in-test', // a guard that breaks is still a wait
  ]);
  const all = (await messages(timerLoopInTest, root)).join('\n');
  assert.match(all, /wait with `until\(ok\)`/);
  assert.match(all, /15\.6 ms/);
  assert.match(all, /check:allow timer-loop-in-test <reason>/);
});
