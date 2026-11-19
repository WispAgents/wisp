// Audit B #4: an element first observed while a kit ResizeObserver's pass runs (mounted by what that callback started,
// in the promise jobs right after it) is observed on the next frame, so WebKit never skips it at a shallower depth.
import { afterEach, expect, test, vi } from 'vitest';
import { inObserverPass, markObserverPass, observeSoon } from '../src/core/observer-scope.ts';

afterEach(() => vi.useRealTimers());

test('a_pass_lasts_until_the_next_task_and_a_new_observation_in_it_waits_a_frame', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'requestAnimationFrame'] });
  const seen: string[] = [];
  expect(inObserverPass()).toBe(false);
  observeSoon(() => seen.push('outside'), () => true);
  expect(seen, 'outside a pass: at once').toEqual(['outside']);
  markObserverPass();
  markObserverPass();
  expect(inObserverPass()).toBe(true);
  // a job the callback started (a lazy part resolving) mounts a list: still inside the pass
  await Promise.resolve();
  expect(inObserverPass()).toBe(true);
  const there = true;
  observeSoon(() => seen.push('in pass'), () => there);
  observeSoon(() => seen.push('gone by then'), () => false);
  expect(seen).toEqual(['outside']);
  vi.advanceTimersByTime(0);
  expect(inObserverPass(), 'over at the next task').toBe(false);
  vi.advanceTimersToNextFrame();
  expect(seen, 'on the next frame, only what is still there').toEqual(['outside', 'in pass']);
});
