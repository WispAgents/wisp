// The feed row's time width (B30): the quick actions sit just before the time, so a wide time ("16:24", "Mon 9:00")
// never sits under Mark done. v1 placed them at fixed offsets (right 40px, 68px) that fit only a short time ("2h").
// One ResizeObserver for every row: it reports each time's width after layout (no layout read of ours), and the row
// gets it as `--k-time-w`, which the feed CSS adds to the actions' offsets and the title's room.

const rows = new WeakMap<Element, HTMLElement>();
/** The width last written on each row: a callback that reports the same width writes nothing, so the row's style
 * isn't invalidated (perf audit 2026-11-18 #13: every row was rewritten on each observer callback). */
const written = new WeakMap<HTMLElement, number>();
const observer =
  typeof ResizeObserver === 'undefined'
    ? undefined
    : new ResizeObserver((entries) => {
        for (const e of entries) {
          const row = rows.get(e.target);
          const w = Math.ceil(e.borderBoxSize[0]?.inlineSize ?? e.contentRect.width);
          if (!row || written.get(row) === w) continue;
          written.set(row, w);
          row.style.setProperty('--k-time-w', `${w}px`);
        }
      });

/** Watch `time`'s width for `row`; the returned function stops (and clears the row's width). */
export function watchTimeWidth(row: HTMLElement, time: Element): () => void {
  if (!observer) return () => {};
  rows.set(time, row);
  observer.observe(time);
  return () => {
    observer.unobserve(time);
    rows.delete(time);
    written.delete(row);
    row.style.removeProperty('--k-time-w');
  };
}
