// The FAB gives way (v1 web/js/features/fab.js giveWay): while any of `selector` (a row's question
// choices) is on screen inside `root`, `onAway(true)`; it never sits on a Yes. Choices come and go
// with the rows, so a MutationObserver keeps the IntersectionObserver's watch list in step; rows repaint
// their head only when it changes, so it fires rarely. Returns the stop function.
export function watchGiveWay(root: Element, selector: string, onAway: (away: boolean) => void): () => void {
  if (typeof IntersectionObserver === 'undefined' || typeof MutationObserver === 'undefined') return () => {};
  const seen = new Set<Element>();
  const watched = new Set<Element>();
  let away = false;
  const paint = (): void => {
    const now = seen.size > 0;
    if (now === away) return;
    away = now;
    onAway(now);
  };
  const io = new IntersectionObserver((entries) => {
    for (const en of entries) {
      if (en.isIntersecting) seen.add(en.target);
      else seen.delete(en.target);
    }
    paint();
  });
  const sync = (): void => {
    const now = new Set(root.querySelectorAll(selector));
    for (const c of watched) {
      if (now.has(c)) continue;
      io.unobserve(c);
      watched.delete(c);
      seen.delete(c);
    }
    for (const c of now) {
      if (watched.has(c)) continue;
      watched.add(c);
      io.observe(c);
    }
    paint();
  };
  const mo = new MutationObserver(sync);
  mo.observe(root, { childList: true, subtree: true });
  sync();
  return () => {
    mo.disconnect();
    io.disconnect();
  };
}
