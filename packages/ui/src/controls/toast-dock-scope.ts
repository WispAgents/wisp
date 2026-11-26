// The toast stack's clearance (v1 web/js/ui/toast.js clearDock), a scope helper: while toasts show, the stack sits 10px
// over a box at the bottom that crosses the middle (a reply box, New task's box, the crew editor's foot), so a toast
// never covers Send (B18). It centres the stack on the prompt box when one is drawn (a task's reply box, New task's
// box), else on the main pane, not the window, so neither the sidebar (open, dragged wider or collapsed) nor a side
// pane beside the box pulls it off the input the user types in. It measures every 200 ms, as v1 did,
// and stops on dispose (tools/check `state`).

/** What a toast stack keeps clear of. */
export const DOCKS = "[data-kit='reply-box'], [data-kit='launcher'], [data-kit='ce-foot']";

/** The prompt boxes the stack centres on when one is drawn: a task's reply box, New task's box. */
export const PROMPTS = "[data-kit='reply-box'], [data-kit='launcher']";

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

/** The x (px) the stack centres on: the middle of the lowest prompt box drawn inside the main pane (or the window,
 *  with no main pane), else centreX's. */
export function promptX(w: number, main: Pick<DOMRect, 'left' | 'right' | 'width'> | null | undefined, prompts: readonly Rect[]): number {
  const [lo, hi] = onScreen(w, main) ? [main.left, main.right] : [0, w];
  let best: Rect | undefined;
  for (const r of prompts) {
    const mid = r.left + r.width / 2;
    if (r.width && mid > lo && mid < hi && (!best || r.bottom > best.bottom)) best = r;
  }
  return best ? best.left + best.width / 2 : centreX(w, main);
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
    const cx = promptX(w, main, [...document.querySelectorAll<HTMLElement>(PROMPTS)].map((c) => c.getBoundingClientRect()));
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
