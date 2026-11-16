// F-phone-8: a tap on a phone's list answers in its own frame. Building the whole task page in the tap (its turns,
// the reply box, the details: ~50 ms of script, ~20 ms of layout and ~14 ms of paint on a 4× slowed phone) put its
// first frame at ~105 ms against a 50 ms budget. So on a stack the tap's frame draws the page (its head with Back, the
// empty body) and the body is built in the frame after (design/feedback.md: an immediate navigation, then its
// content). Split builds it all at once, as before: the list stays beside it and the reply box takes the focus.

/** Runs `fn` in the frame after the next one, so whatever is on screen now is painted first. Returns a cancel. */
export function afterNextPaint(fn: () => void, raf: (f: FrameRequestCallback) => number = requestAnimationFrame, cancel: (id: number) => void = cancelAnimationFrame): () => void {
  let id = raf(() => {
    id = raf(() => fn());
  });
  return () => cancel(id);
}

/** Whether the task page builds its body in its first frame: in split, or where nothing paints (no frames to wait). */
export const buildsAtOnce = (stack: boolean, frames: boolean): boolean => !stack || !frames;
