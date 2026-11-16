// F-phone-8: on a phone the tap's frame draws the task page's head; its body is built in the frame after.
import { describe, expect, it } from 'vitest';
import { afterNextPaint, buildsAtOnce } from '../src/frame/task-paint.ts';

/** Frames the test steps through by hand. */
function frames(): { readonly raf: (f: FrameRequestCallback) => number; readonly cancel: (id: number) => void; readonly step: () => void } {
  let next = 1;
  let queue = new Map<number, FrameRequestCallback>();
  return {
    raf: (f) => (queue.set(next, f), next++),
    cancel: (id) => void queue.delete(id),
    step: () => {
      const now = queue;
      queue = new Map();
      for (const f of now.values()) f(0);
    },
  };
}

describe('the task page after the tap', () => {
  it('the_body_comes_in_the_frame_after_the_one_that_shows_the_page', () => {
    const f = frames();
    let ran = 0;
    afterNextPaint(() => ran++, f.raf, f.cancel);
    f.step(); // the tap's frame: the head is painted
    expect(ran).toBe(0);
    f.step(); // the next one: the body
    expect(ran).toBe(1);
    f.step();
    expect(ran).toBe(1);
  });

  it('a_page_closed_before_its_body_came_never_builds_it', () => {
    const f = frames();
    let ran = 0;
    const early = afterNextPaint(() => ran++, f.raf, f.cancel);
    early(); // closed before the first frame
    const late = afterNextPaint(() => ran++, f.raf, f.cancel);
    f.step();
    late(); // closed between the two frames
    f.step();
    f.step();
    expect(ran).toBe(0);
  });

  it('split_and_frameless_pages_build_all_at_once', () => {
    expect(buildsAtOnce(false, true)).toBe(true);
    expect(buildsAtOnce(true, false)).toBe(true);
    expect(buildsAtOnce(true, true)).toBe(false);
  });
});
