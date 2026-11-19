// The touch binders (bind.ts, bind-vertical.ts), loaded apart (ADR-0105): only a phone look binds a gesture (use.ts
// returns before binding in the browser, at any width), so the browser never fetches them. The phone apps load them
// with their look (platform/styles.ts), so a gesture binds as its element mounts; one that mounts first binds the
// moment they land.
import { lazyPart } from '../controls/lazy-part.ts';

export type Binders = typeof import('./binders.ts');

export const binders = lazyPart(() => import('./binders.ts'), { preload: false });

/** Binds with the binders: at once when they're here, else once they land. The stop it returns works either way (a
 *  stop before they land binds nothing). */
export function bindLater(bind: (b: Binders) => () => void): () => void {
  const now = binders.ready();
  if (now) return bind(now);
  const st: { stop: (() => void) | undefined; off: boolean } = { stop: undefined, off: false };
  binders.load().then(
    (b) => {
      if (!st.off) st.stop = bind(b);
    },
    () => undefined,
  );
  return () => {
    st.off = true;
    st.stop?.();
  };
}
