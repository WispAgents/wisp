// The page's web fonts, loaded: one promise for the page. Reading `document.fonts.ready` updates the style first (8.5 ms
// each time the task ledger mounted, in a Mark done's frame: the flicker gate's profile), and fonts load once a page,
// so the first read is kept and every later one is free.

const kept = new WeakMap<FontFaceSet, Promise<unknown>>();

export function fontsReady(): Promise<unknown> {
  const set = typeof document === 'undefined' ? undefined : document.fonts;
  if (!set) return Promise.resolve();
  let p = kept.get(set);
  if (!p) {
    p = set.ready;
    kept.set(set, p);
  }
  return p;
}
