// useDerived (core/signal.ts, round 323): a Solid memo that also follows the alien signals it reads. The palette's items
// read the entity store (alien) and a source's own Solid state (Open file's list, New task's pick) on one pass.
import { expect, test } from 'vitest';
import { effect, signal } from 'alien-signals';
import { createRoot, createSignal, type Accessor } from 'solid-js';
import { useDerived } from '../src/core/signal.ts';

test('a_derived_value_follows_alien_and_solid_reads_with_one_run_per_change', () => {
  const store = signal<readonly string[]>([]);
  const [own, setOwn] = createSignal('a');
  let runs = 0;
  let v: Accessor<string> | undefined;
  const dispose = createRoot((d) => {
    v = useDerived(() => (runs++, `${own()}:${store().join(',')}`));
    return d;
  });
  expect(v?.()).toBe('a:');
  expect(runs, 'one run to start').toBe(1);
  store(['t1']);
  expect(v?.(), 'the store’s late rows').toBe('a:t1');
  expect(runs).toBe(2);
  setOwn('b');
  expect(v?.(), 'the source’s own Solid state').toBe('b:t1');
  expect(runs).toBe(3);
  store(['t1', 't2']);
  expect(v?.(), 'still following the store after a Solid-led run').toBe('b:t1,t2');
  expect(runs).toBe(4);
  dispose();
  store(['gone']);
  expect(runs, 'disposed: nothing runs').toBe(4);
});

test('a_derived_value_follows_the_reads_of_its_latest_run_when_a_branch_switches_them', () => {
  const which = signal<'x' | 'y'>('x');
  const x = signal(1);
  const y = signal(10);
  let v: Accessor<number> | undefined;
  let runs = 0;
  const dispose = createRoot((d) => {
    v = useDerived(() => (runs++, which() === 'x' ? x() : y()));
    return d;
  });
  which('y');
  expect(v?.()).toBe(10);
  y(11);
  expect(v?.(), 'the branch it reads now').toBe(11);
  expect(runs).toBe(3);
  x(2);
  expect(runs, 'the branch it left no longer wakes it').toBe(3);
  dispose();
});

test('a_derived_value_made_while_another_alien_effect_runs_keeps_following_after_that_effect_runs_again', () => {
  const outer = signal(0);
  const store = signal(0);
  let v: Accessor<number> | undefined;
  let dispose: (() => void) | undefined;
  const stopOuter = effect(() => {
    if (outer() === 1 && !v)
      createRoot((d) => {
        dispose = d;
        v = useDerived(() => store());
      });
  });
  outer(1);
  outer(2);
  store(5);
  expect(v?.()).toBe(5);
  stopOuter();
  dispose?.();
});

test('a_derived_value_whose_fn_throws_throws_from_the_memo_and_leaves_no_watcher_running', () => {
  const store = signal(0);
  let runs = 0;
  let caught: unknown;
  const dispose = createRoot((d) => {
    try {
      useDerived(() => {
        runs++;
        store();
        throw new Error('bad source');
      });
    } catch (err) {
      caught = err;
    }
    return d;
  });
  expect((caught as Error | undefined)?.message).toBe('bad source');
  expect(runs).toBe(1);
  // the failed pass's watcher is gone: a store write doesn't run fn behind the memo's back, nor throw from the write
  expect(() => store(1)).not.toThrow();
  expect(runs).toBe(1);
  dispose();
});
