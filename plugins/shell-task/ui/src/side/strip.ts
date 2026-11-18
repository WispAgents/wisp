// UX 2026-11-18 desktop #5: the side panel's tab strip holds the tabs with something in them (a diff, a PR, a running
// preview, a pinned pane), and the one open; the rest wait behind + at its end. A tab that joined stays (it never
// jumps out when its content goes, say a diff committed away), in the panel's own order. DOM-free.

/** The strip and what's behind +: `order` the panel's tabs in order, `has` those with something in them, `joined` the
 *  strip as it stood (what joined never leaves while the panel still has it). */
export function sideStrip(i: { readonly order: readonly string[]; readonly has: ReadonlySet<string>; readonly current: string; readonly joined: readonly string[] }): {
  strip: string[];
  more: string[];
} {
  const strip = i.order.filter((p) => i.has.has(p) || p === i.current || i.joined.includes(p));
  return { strip, more: i.order.filter((p) => !strip.includes(p)) };
}

type Checks = { readonly pass?: number; readonly fail?: number; readonly pending?: number };

/** What a tab says beside its name: the PR's number and its checks (✕ one failed, ● some running, ✓ all passed). */
export const tabCount = {
  pr(pr: { readonly number: number; readonly checks?: Checks } | undefined): string | undefined {
    if (!pr) return undefined;
    const c = pr.checks;
    const mark = !c ? '' : (c.fail ?? 0) > 0 ? ' ✕' : (c.pending ?? 0) > 0 ? ' ●' : (c.pass ?? 0) > 0 ? ' ✓' : '';
    return `#${pr.number}${mark}`;
  },
};
