// What keeps the side panel mounted (perf r2 #8). Each of its tabs (Changes, PR, Shell, Preview, Watch) is its own area
// (`shell.side:<kind>`), so keyed by kind a tab switch built the whole panel again (its head, tabs, place row and body:
// 96–123 ms at 4×). The same code drawing the next tab for the same task keeps it: that panel reads which tab is open
// from navigation and swaps only its body. Another plugin's code for a tab is another panel, built afresh.

/** What an area's last extension is, for telling whether two tabs are drawn by the same code. */
export interface DrawnBy {
  readonly plugin: string;
  readonly component: object;
  readonly config: unknown;
}

/** The side pane's key: `''` closed, a view by its id, a task's tab by the code that draws it and the task. */
export function sideKey(side: { readonly kind: string; readonly id?: unknown }, drawnBy: (kind: string) => DrawnBy | undefined, idOf: (component: object) => number): string {
  if (side.kind === 'none') return '';
  if (side.kind === 'view') return `view:${String(side.id)}`;
  const d = drawnBy(side.kind);
  const who = d ? `${d.plugin}#${idOf(d.component)}#${JSON.stringify(d.config)}` : side.kind;
  return `${who}:${String(side.id)}`;
}

/** A small number per component, the same for the same one. */
export function componentIds(): (component: object) => number {
  const ids = new WeakMap<object, number>();
  let n = 0;
  return (c) => {
    let id = ids.get(c);
    if (id === undefined) ids.set(c, (id = ++n));
    return id;
  };
}
