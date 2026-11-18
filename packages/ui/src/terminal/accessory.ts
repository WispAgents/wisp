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

/** Keeps `--k-kb` on `el` at the keyboard's height while it is bound. Returns the stop. */
export function bindAccessory(el: HTMLElement, w: ViewportWindow): () => void {
  const write = (): void => el.style.setProperty('--k-kb', `${keyboardInset(w)}px`);
  write();
  const v = w.visualViewport;
  v?.addEventListener('resize', write);
  v?.addEventListener('scroll', write);
  return () => {
    v?.removeEventListener('resize', write);
    v?.removeEventListener('scroll', write);
    el.style.removeProperty('--k-kb');
  };
}
