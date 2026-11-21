// What a press draws first (design/feedback.md, ADR-0017): its cheap answer (a highlight, a title, a skeleton) is
// painted in the press's own frame, and what costs (a pane's rows, a page's body) is built in the frame after. The
// shell's task page has done this since perf r1 #6 (shell task-paint.ts); this is the kit's, for any plugin: perf r2 #9
// (a Settings pane opened again built in the click, 120–160 ms at 4×).
import { createEffect, createSignal, on, onCleanup, untrack, type Accessor } from 'solid-js';

type Raf = (f: FrameRequestCallback) => number;

/** Runs `fn` in the frame after the next one, so whatever is on screen now is painted first. Returns a cancel. */
export function afterNextPaint(fn: () => void, raf: Raf = requestAnimationFrame, cancel: (id: number) => void = cancelAnimationFrame): () => void {
  let id = raf(() => {
    id = raf(() => fn());
  });
  return () => cancel(id);
}

/** Whether there are frames to wait for (a browser); without them (a test, a server render) nothing is deferred. */
const painting = (): boolean => typeof requestAnimationFrame === 'function';

/**
 * `source`, one painted frame behind each change: the first value at once, each later one once the frame that showed
 * the change's cheap part is painted. A change that comes back before then cancels the one waiting. `equals` says
 * when a new value is the one already shown (nothing to rebuild).
 */
export function useAfterPaint<T>(source: Accessor<T>, equals: (a: T, b: T) => boolean = Object.is): Accessor<T> {
  const [shown, setShown] = createSignal<T>(untrack(source), { equals });
  if (!painting()) return source;
  let cancel: (() => void) | undefined;
  createEffect(
    on(
      source,
      (v) => {
        cancel?.();
        cancel = undefined;
        if (equals(v, untrack(shown))) return;
        cancel = afterNextPaint(() => {
          cancel = undefined;
          setShown(() => v);
        });
      },
      { defer: true },
    ),
  );
  onCleanup(() => cancel?.());
  return shown;
}

/**
 * How many of `total()` items to draw: `first` at once, then `step` more each frame until all are there, so a long
 * list (All settings' rows) is built across frames instead of in one press. Without frames, all of them at once.
 */
export function useProgressive(total: Accessor<number>, first: number, step: number = first): Accessor<number> {
  if (!painting()) return total;
  const [shown, setShown] = createSignal(first);
  let id: number | undefined;
  const more = (): void => {
    id = undefined;
    const n = untrack(shown) + step;
    setShown(n);
    if (n < untrack(total)) id = requestAnimationFrame(more);
  };
  createEffect(
    on(total, (n) => {
      if (id === undefined && untrack(shown) < n) id = requestAnimationFrame(more);
    }),
  );
  onCleanup(() => id !== undefined && cancelAnimationFrame(id));
  return () => Math.min(shown(), total());
}
