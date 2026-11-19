// While the browser delivers ResizeObserver callbacks, what a kit observer's callback starts (a signal write, and the
// promise jobs that run right after it, such as a lazy part that has just loaded) can mount new elements. An element
// observed then that sits no deeper than what this pass already measured is skipped, and WebKit reports "ResizeObserver
// loop completed with undelivered notifications" (audit B #4: the phone's task list mounting right after the top bar's
// Toolbar measured itself). The kit's observers mark their pass here; an element first observed during one is observed
// on the next frame instead, where its first size is delivered like any other. (A scope helper: its one timer ends
// the pass at the next task and can't outlive it.)

const pass = { open: false };

/** Called at the top of a kit ResizeObserver callback: the pass lasts until the next task. */
export function markObserverPass(): void {
  if (pass.open) return;
  pass.open = true;
  setTimeout(() => {
    pass.open = false;
  }, 0);
}

/** A kit observer's callback, or a job it started, is running in this pass. */
export const inObserverPass = (): boolean => pass.open;

/** `observe(el)` now, or on the next frame when inside an observer pass (and only if `el` is still there then). */
export function observeSoon(observe: () => void, still: () => boolean): void {
  if (!pass.open) return observe();
  requestAnimationFrame(() => {
    if (still()) observe();
  });
}
