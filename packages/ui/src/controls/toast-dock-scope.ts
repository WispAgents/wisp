// The toast stack's clearance (v1 web/js/ui/toast.js clearDock), a scope helper: while toasts show, the stack sits 10px
// over a box at the bottom that crosses the middle (a reply box, New task's box, the crew editor's foot), so a toast
// never covers Send (B18). It also centres the stack on the main pane, not the window, so the sidebar (open, dragged
// wider or collapsed) never pulls it off the middle of the page the user reads. It measures every 200 ms, as v1 did,
// and stops on dispose (tools/check `state`).

/** What a toast stack keeps clear of. */
export const DOCKS = "[data-kit='reply-box'], [data-kit='launcher'], [data-kit='ce-foot']";

/** The pane the stack centres on: the app's main pane (Stage.tsx's split-main, Frame.tsx's main pane). */
export const MAIN = "[data-kit='split-main'], [data-kit='frame-pane'][data-pane='main']";

type Rect = Pick<DOMRect, 'top' | 'bottom' | 'left' | 'right' | 'width'>;

/** The main pane's rect when it is drawn inside the window; a phone's stack slides it off-screen on the list page. */
function onScreen(w: number, main: Pick<DOMRect, 'left' | 'right' | 'width'> | null | undefined): main is Pick<DOMRect, 'left' | 'right' | 'width'> {
  return !!main && main.width > 0 && main.left >= -1 && main.right <= w + 1;
}

/** The x (px) the stack centres on: the middle of the main pane when it is drawn, else the window's middle. */
export function centreX(w: number, main: Pick<DOMRect, 'left' | 'right' | 'width'> | null | undefined): number {
  return onScreen(w, main) ? main.left + main.width / 2 : w / 2;
}

/** The width the stack may take (CSS): the main pane's, else the window's. */
export function roomFor(w: number, main: Pick<DOMRect, 'left' | 'right' | 'width'> | null | undefined): string {
  return onScreen(w, main) ? `${Math.round(main.width)}px` : '100vw';
}

/** The stack's `--toast-bottom` (px) for a window `h` tall and `w` wide and the boxes' rects; '' when none is in the way.
 *  `cx` is the stack's centre (centreX), the window's middle by default. */
export function dockBottom(h: number, w: number, boxes: readonly Rect[], cx = w / 2): string {
  let top = h;
  for (const r of boxes) if (r.width && r.left < cx && r.right > cx && r.bottom > h - 140 && r.top < h) top = Math.min(top, r.top);
  return top < h ? `${Math.round(h - top + 10)}px` : '';
}

/** Keeps `group` clear of the docks and centred on the main pane until the returned dispose runs. */
export function dockToasts(group: HTMLElement): () => void {
  const dock = (): void => {
    const w = window.innerWidth;
    const main = document.querySelector<HTMLElement>(MAIN)?.getBoundingClientRect();
    const cx = centreX(w, main);
    const boxes = [...document.querySelectorAll<HTMLElement>(DOCKS)].map((c) => c.getBoundingClientRect());
    group.style.setProperty('--toast-bottom', dockBottom(window.innerHeight, w, boxes, cx));
    group.style.setProperty('--toast-x', `${Math.round(cx)}px`);
    group.style.setProperty('--toast-room', roomFor(w, main));
  };
  dock();
  const every = setInterval(dock, 200);
  // Collapsing or dragging the sidebar resizes the main pane: follow it in the same frame, not on the next tick.
  const pane = document.querySelector<HTMLElement>(MAIN);
  const seen = pane && typeof ResizeObserver === 'function' ? new ResizeObserver(dock) : null;
  if (pane) seen?.observe(pane);
  return () => {
    clearInterval(every);
    seen?.disconnect();
  };
}
