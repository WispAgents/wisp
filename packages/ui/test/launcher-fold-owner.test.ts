// QA round 6 desktop #3 / phone #9: "computations created outside a createRoot" on every Home load and Settings open.
// The bar's measure runs in a frame callback, outside any owner, and read the launcher's `tail` prop there: the host's
// getter builds its tools (memos) on each read, so each frame made computations nobody owned. The hook reads the tail
// through a memo it owns; a frame's measure only reads values.
import { expect, test, vi } from 'vitest';
import { createRoot, getOwner } from 'solid-js';
import { useTailFold } from '../src/app/launcher-fold-measure.ts';

test('the_tail_is_never_read_outside_an_owner_qa_r6_3', () => {
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal(
    'ResizeObserver',
    class {
      readonly cb: () => void;
      constructor(cb: () => void) {
        this.cb = cb;
      }
      observe(): void {
        this.cb();
      }
      disconnect(): void {}
      unobserve(): void {}
    },
  );
  vi.stubGlobal('MutationObserver', class { observe(): void {} disconnect(): void {} });
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => frames.push(f));
  const owners: boolean[] = [];
  const bar = document.createElement('div');
  document.body.append(bar);
  const dispose = createRoot((d) => {
    const fold = useTailFold({ tail: () => (owners.push(getOwner() !== null), []), on: () => true });
    fold.ref(bar);
    return d;
  });
  // the frame the observer asked for runs later, with no owner
  expect(frames.length).toBeGreaterThan(0);
  for (const f of frames.splice(0)) f(0);
  expect(owners.length, 'the tail was read').toBeGreaterThan(0);
  expect(owners.every(Boolean), 'every read of the tail had an owner').toBe(true);
  dispose();
  vi.unstubAllGlobals();
});
