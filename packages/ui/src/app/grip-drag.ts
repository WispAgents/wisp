// The frame's resize grips (Stage: the task list's edge, the side panel's): a press that drags a pane's width.

/** A grip's press: the pane's width follows the pointer (`dir` 1: it grows to the right, the list; -1: to the left,
 *  the side panel) from what it was, reported on every move and once as it's let go (v1 split.js and panel.js grips:
 *  the pointer captured, `resizing` while it moves). The window hears the moves, so a grip that goes away mid-drag (the
 *  panel closed, `[`) still ends it; a cancel keeps the last width; a press that never moved reports nothing. Only the
 *  press reads layout, and only when no width was given. */
export function drag(width: () => number | undefined, dir: 1 | -1, report: (px: number, done: boolean) => void): (e: PointerEvent) => void {
  return (e) => {
    if (e.button !== 0 || !e.isPrimary) return;
    const grip = e.currentTarget as HTMLElement;
    const stage = grip.closest<HTMLElement>('[data-kit="stage"]');
    // one drag at a time (a second finger on a grip)
    if (stage?.hasAttribute('data-resizing')) return;
    const pane = grip.parentElement?.querySelector<HTMLElement>(dir === 1 ? ':scope > [data-kit="feed-wrap"]' : ':scope > [data-kit="side-pane"]');
    const start = width() ?? Math.round(pane?.getBoundingClientRect().width ?? 0);
    const from = e.clientX;
    const id = e.pointerId;
    let last: number | undefined;
    e.preventDefault();
    grip.setPointerCapture?.(id);
    stage?.setAttribute('data-resizing', '');
    const move = (ev: PointerEvent): void => {
      if (ev.pointerId !== id) return;
      last = Math.round(start + dir * (ev.clientX - from));
      report(last, false);
    };
    const end = (ev: PointerEvent): void => {
      if (ev.pointerId !== id) return;
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', end);
      stage?.removeAttribute('data-resizing');
      // a cancel's position means nothing (the OS took the pointer): the last move stands
      if (ev.type === 'pointerup' && ev.clientX !== from) last = Math.round(start + dir * (ev.clientX - from));
      if (last !== undefined) report(last, true);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
  };
}
