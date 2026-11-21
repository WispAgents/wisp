// The side panel's strip (owner, 2026-11-19, replacing UX 2026-11-18 desktop #5's "tabs only when they have something,
// the rest behind +"): v1 panel.js paintName's tabs, shown by default and labelled whether or not they have anything yet,
// in a fixed order so none ever moves: a terminal task's CLI first, Changes, PR, Shell, Preview, Watch while the task has
// one, then the tabs other plugins pin (My Stuff apps, Thread). Labels give way to icons only when the panel has no room
// for them (the kit's Tabs `fit`). DOM-free.

/** The panel's tabs, in order: `term` a terminal task's CLI, `watch` the task has a Watch page, `extras` pinned tabs. */
export function sideStrip(i: { readonly term: boolean; readonly watch: boolean; readonly extras: readonly string[] }): string[] {
  return [...(i.term ? ['term'] : []), 'changes', 'pr', 'shell', 'preview', ...(i.watch ? ['watch'] : []), ...i.extras];
}

type Checks = { readonly pass?: number; readonly fail?: number; readonly pending?: number };

/** What a tab says: the PR tab its name and number as v1 (`PR #12`, panel.js paintName; `word` is the tab's name) and
 *  its checks (✕ one failed, ● some running, ✓ all passed). */
export const tabCount = {
  pr(pr: { readonly number: number; readonly checks?: Checks } | undefined, word: string): string | undefined {
    if (!pr) return undefined;
    const c = pr.checks;
    const mark = !c ? '' : (c.fail ?? 0) > 0 ? ' ✕' : (c.pending ?? 0) > 0 ? ' ●' : (c.pass ?? 0) > 0 ? ' ✓' : '';
    return `${word} #${pr.number}${mark}`;
  },
};
