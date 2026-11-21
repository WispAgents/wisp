// A bar's labels going to their icons by measure (design/layout.md §2, ADR-0012: a bar folds, it never clips): the
// reply box's tools ran 3 px past a 360 px Android screen and the bar scrolled. While its items with their words are
// wider than the bar, `tight` is on (the CSS drops the words, the tooltips and aria-labels keep them); the width they
// need is remembered, so they come back once the bar holds it, with no flapping at the edge. One ResizeObserver on the
// bar and its items, read once a frame.
import { createSignal, onCleanup, type Accessor } from 'solid-js';

/** Whether the labels go: the width the items need with them (`need`, undefined until measured) against the bar's. */
export const labelsGo = (need: number | undefined, available: number): boolean => need !== undefined && need > available + 0.5;

/** the items' widths and the gaps between them; one drawn at 0 px (the grow spacer) still takes its gaps */
function contentWidth(bar: HTMLElement): number {
  let sum = 0;
  let n = 0;
  for (const el of bar.children) {
    if (!(el instanceof HTMLElement) || el.getClientRects().length === 0) continue;
    // the grow spacer: its gaps, never its width (it takes what's left)
    sum += el.dataset['kit'] === 'grow' ? 0 : el.offsetWidth;
    n++;
  }
  return sum + (Number.parseFloat(getComputedStyle(bar).columnGap) || 0) * Math.max(0, n - 1);
}

export function useBarLabels(): { readonly ref: (el: HTMLElement) => void; readonly tight: Accessor<boolean> } {
  const [tight, setTight] = createSignal(false);
  if (typeof ResizeObserver === 'undefined' || typeof MutationObserver === 'undefined') return { ref: () => {}, tight };
  let bar: HTMLElement | undefined;
  let need: number | undefined;
  let frame = 0;
  const measure = (): void => {
    frame = 0;
    if (!bar?.isConnected) return;
    // with their words: what they need now; without, the need last seen stands (the words are what it's about)
    if (!tight()) need = contentWidth(bar);
    const cs = getComputedStyle(bar);
    // its content box: a bar's own padding is no room for its items
    const room = bar.clientWidth - (Number.parseFloat(cs.paddingLeft) || 0) - (Number.parseFloat(cs.paddingRight) || 0);
    const next = labelsGo(need, room);
    if (next === tight()) return;
    setTight(next);
    soon();
  };
  const soon = (): void => {
    if (frame === 0) frame = requestAnimationFrame(measure);
  };
  const ro = new ResizeObserver(soon);
  const mo = new MutationObserver((records) => {
    for (const r of records) {
      for (const n of r.addedNodes) if (n instanceof HTMLElement) ro.observe(n);
      for (const n of r.removedNodes) if (n instanceof HTMLElement) ro.unobserve(n);
    }
    // an item came or went: what the words need is measured again, with them
    if (tight()) {
      need = undefined;
      setTight(false);
    }
    soon();
  });
  onCleanup(() => {
    ro.disconnect();
    mo.disconnect();
    if (frame !== 0) cancelAnimationFrame(frame);
  });
  return {
    ref: (el) => {
      bar = el;
      ro.observe(el);
      for (const kid of el.children) ro.observe(kid);
      mo.observe(el, { childList: true });
    },
    tight,
  };
}
