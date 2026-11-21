// The toast stack's clearance (v1 web/js/ui/toast.js clearDock), a scope helper: while toasts show, the stack sits 10px
// over a box at the bottom that crosses the middle (a reply box, New task's box, the crew editor's foot), so a toast
// never covers Send (B18). It measures every 200 ms, as v1 did, and stops on dispose (tools/check `state`).

/** What a toast stack keeps clear of. */
export const DOCKS = "[data-kit='reply-box'], [data-kit='launcher'], [data-kit='ce-foot']";

/** The stack's `--toast-bottom` (px) for a window `h` tall and `w` wide and the boxes' rects; '' when none is in the way. */
export function dockBottom(h: number, w: number, boxes: readonly Pick<DOMRect, 'top' | 'bottom' | 'left' | 'right' | 'width'>[]): string {
  const cx = w / 2;
  let top = h;
  for (const r of boxes) if (r.width && r.left < cx && r.right > cx && r.bottom > h - 140 && r.top < h) top = Math.min(top, r.top);
  return top < h ? `${Math.round(h - top + 10)}px` : '';
}

/** Keeps `group` clear of the docks until the returned dispose runs. */
export function dockToasts(group: HTMLElement): () => void {
  const dock = (): void => {
    const boxes = [...document.querySelectorAll<HTMLElement>(DOCKS)].map((c) => c.getBoundingClientRect());
    group.style.setProperty('--toast-bottom', dockBottom(window.innerHeight, window.innerWidth, boxes));
  };
  dock();
  const every = setInterval(dock, 200);
  return () => clearInterval(every);
}
