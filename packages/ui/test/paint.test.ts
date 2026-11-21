// The kit's paint helpers (perf r2 #9): a press paints its cheap part first, the costly part a painted frame later.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createRoot, createSignal } from 'solid-js';
import { afterNextPaint, useAfterPaint, useProgressive } from '../src/core/paint.ts';

let frames: FrameRequestCallback[] = [];
const was = { raf: globalThis.requestAnimationFrame, cancel: globalThis.cancelAnimationFrame };
const frame = (): void => {
  const now = frames;
  frames = [];
  for (const f of now) f(0);
};

beforeEach(() => {
  frames = [];
  globalThis.requestAnimationFrame = (f) => frames.push(f);
  globalThis.cancelAnimationFrame = (id) => {
    frames[id - 1] = () => undefined;
  };
});
afterEach(() => {
  globalThis.requestAnimationFrame = was.raf;
  globalThis.cancelAnimationFrame = was.cancel;
});

describe('afterNextPaint', () => {
  it('runs in the frame after the next one, and not once cancelled', () => {
    const ran: string[] = [];
    afterNextPaint(() => ran.push('a'));
    const stop = afterNextPaint(() => ran.push('b'));
    frame();
    expect(ran).toEqual([]);
    stop();
    frame();
    expect(ran).toEqual(['a']);
  });
});

/** `make`'s accessors, inside a root whose effects have started (Solid runs them once the root's setup returns). */
function mount<T>(make: () => T): T & { dispose: () => void } {
  return createRoot((dispose) => ({ ...make(), dispose }));
}

describe('useAfterPaint', () => {
  it('shows the first value at once and each change a painted frame later', () => {
    const m = mount(() => {
      const [src, setSrc] = createSignal('general');
      return { setSrc, shown: useAfterPaint(src) };
    });
    expect(m.shown()).toBe('general');
    m.setSrc('usage');
    expect(m.shown()).toBe('general'); // the click's frame: still the old pane, its row already lit
    frame();
    expect(m.shown()).toBe('general');
    frame();
    expect(m.shown()).toBe('usage');
    m.dispose();
  });

  it('a change back before the paint cancels the one waiting', () => {
    const m = mount(() => {
      const [src, setSrc] = createSignal('general');
      return { setSrc, shown: useAfterPaint(src) };
    });
    m.setSrc('usage');
    m.setSrc('general');
    frame();
    frame();
    expect(m.shown()).toBe('general');
    m.setSrc('all');
    m.setSrc('agents');
    frame();
    frame();
    expect(m.shown()).toBe('agents');
    m.dispose();
  });
});

describe('useProgressive', () => {
  it('draws the first rows at once and the rest a step a frame, up to the total', () => {
    const m = mount(() => {
      const [total, setTotal] = createSignal(40);
      return { setTotal, n: useProgressive(total, 16) };
    });
    expect(m.n()).toBe(16);
    frame();
    expect(m.n()).toBe(32);
    frame();
    expect(m.n()).toBe(40);
    frame();
    expect(frames).toHaveLength(0); // done: no frame asked for
    m.setTotal(10);
    expect(m.n()).toBe(10); // a search that cut the list shows what's left at once
    m.setTotal(60);
    frame();
    frame();
    expect(m.n()).toBe(60);
    m.dispose();
  });
});
