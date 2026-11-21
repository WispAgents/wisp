// The Shell's key row as an input accessory on a phone (docs/ux/2026-11-18-phone.md #13): while the terminal has the
// keyboard, the row sits on the keyboard's top edge, as Termius and Prompt keep theirs, wherever the WebView puts the
// page (resized or overlaid). The keyboard's height comes from the visual viewport: what the layout viewport has and
// the visual one doesn't, under it. Pure math plus one binding that writes a CSS variable (no layout reads after it).

/** the parts of `window` it reads; tests pass a fake */
export interface ViewportWindow {
  readonly innerHeight: number;
  readonly visualViewport?: {
    readonly height: number;
    readonly offsetTop: number;
    addEventListener(t: 'resize' | 'scroll', fn: () => void): void;
    removeEventListener(t: 'resize' | 'scroll', fn: () => void): void;
  } | null;
}

/** how much of the layout viewport's bottom the keyboard covers (0 without one, or without a visual viewport) */
export function keyboardInset(w: Pick<ViewportWindow, 'innerHeight' | 'visualViewport'>): number {
  const v = w.visualViewport;
  if (!v) return 0;
  return Math.max(0, Math.round(w.innerHeight - v.height - v.offsetTop));
}

/** the key rows bound now, each with how much keyboard it last saw: the page's flag follows the last one still bound */
const bound = new Map<HTMLElement, number>();

/** The body's `data-kb` and `--k-kb` from the bound rows (the largest inset any of them sees). */
function flag(body: HTMLElement): void {
  const kb = Math.max(0, ...bound.values());
  body.toggleAttribute('data-kb', kb > 0);
  if (kb > 0) body.style.setProperty('--k-kb', `${kb}px`);
  else body.style.removeProperty('--k-kb');
}

/** Keeps `--k-kb` on `el` at the keyboard's height while it is bound, and tells the page while the keyboard is up
 *  (`data-kb` and `--k-kb` on the body: a sheet holding the terminal makes room, QA round 6 phone #1; with two rows
 *  bound, the flag stays until the last lets go). Returns the stop. */
export function bindAccessory(el: HTMLElement, w: ViewportWindow): () => void {
  const body = el.ownerDocument.body;
  const write = (): void => {
    const kb = keyboardInset(w);
    el.style.setProperty('--k-kb', `${kb}px`);
    bound.set(el, kb);
    flag(body);
  };
  write();
  const v = w.visualViewport;
  v?.addEventListener('resize', write);
  v?.addEventListener('scroll', write);
  return () => {
    v?.removeEventListener('resize', write);
    v?.removeEventListener('scroll', write);
    el.style.removeProperty('--k-kb');
    bound.delete(el);
    flag(body);
  };
}
