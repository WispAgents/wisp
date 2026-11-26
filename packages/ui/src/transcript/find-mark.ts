// The marks Find puts on what it found (R4-3): every place the words read in `root`, and the hit it is on, as CSS
// Custom Highlights (`wisp-find`, `wisp-find-current`, find.css). Ranges over the text as drawn, so nothing in the
// page is rewritten and Solid's nodes stay its own. A browser without the API rings the hit's step (`data-find`).

const HL = (): boolean => typeof CSS !== 'undefined' && 'highlights' in CSS && typeof Highlight !== 'undefined';

/** Ranges over each case-insensitive occurrence of `query` in `root`'s text, node by node. */
export function findRanges(root: Node, query: string): Range[] {
  const q = query.toLowerCase();
  if (!q) return [];
  const out: Range[] = [];
  const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walk.nextNode(); n; n = walk.nextNode()) {
    const text = (n.nodeValue ?? '').toLowerCase();
    for (let at = text.indexOf(q); at >= 0; at = text.indexOf(q, at + q.length)) {
      const r = document.createRange();
      r.setStart(n, at);
      r.setEnd(n, at + q.length);
      out.push(r);
    }
  }
  return out;
}

/** Marks `query` across `scope` and, brighter, inside `current` (the hit's step); returns what takes the marks off. */
export function markFind(scope: Element, current: Element | undefined, query: string): () => void {
  if (HL()) {
    const mine = current ? findRanges(current, query) : [];
    const rest = findRanges(scope, query).filter((r) => !current?.contains(r.startContainer));
    CSS.highlights.set('wisp-find', new Highlight(...rest));
    CSS.highlights.set('wisp-find-current', new Highlight(...mine));
    return () => {
      CSS.highlights.delete('wisp-find');
      CSS.highlights.delete('wisp-find-current');
    };
  }
  current?.setAttribute('data-find', '');
  return () => current?.removeAttribute('data-find');
}
