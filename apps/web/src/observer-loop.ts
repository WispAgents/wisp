// WebKit's "ResizeObserver loop completed with undelivered notifications" (audit B #4, audit C #6). The browser
// reports it when a size changes after its observer was told this frame, at a depth it can't tell again in the same
// frame; the spec then tells it on the next frame, so nothing is lost. In wisp what changes sizes then is the app's
// lazy parts landing: a chunk's promise settles between two observer callbacks (the reply box, a turn's blocks) and
// draws there. The kit keeps its own observers out of it (core/observer-scope.ts); a chunk landing can't be timed, so
// this notice, and only it, is handled here instead of reported as a page error. Any other error goes on as before.

const NOTICE = /^ResizeObserver loop (completed with undelivered notifications|limit exceeded)/;

/** Whether `message` is the browser's ResizeObserver loop notice (WebKit's and Chrome's wording). */
export const isObserverLoopNotice = (message: unknown): boolean => typeof message === 'string' && NOTICE.test(message);

/** Handles the loop notice on `win`, first (capture); returns the undo. */
export function quietObserverLoop(win: Pick<Window, 'addEventListener' | 'removeEventListener'>): () => void {
  const on = (e: Event): void => {
    if (!isObserverLoopNotice((e as ErrorEvent).message)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
  };
  win.addEventListener('error', on, true);
  return () => win.removeEventListener('error', on, true);
}
