// Whether a task can be shipped at all (v1 features/ship.js shippable's first test, routes/ship.js hasTrees): Ship it
// commits, pushes and opens a PR from a worktree wisp made, so a task that runs in its project's own folder has no
// Ship (audit B #2: its head offered one that failed with "no such session"). `branch` is wisp.worktrees' row field,
// set only for a task with a worktree; `also` holds a task's other folders. Pure.

export function shipsFromWorktree(summary: Readonly<Record<string, unknown>> | undefined): boolean {
  if (!summary) return false;
  const named = (b: unknown): boolean => typeof b === 'string' && b !== '';
  const also = Array.isArray(summary['also']) ? (summary['also'] as readonly { readonly branch?: unknown }[]) : [];
  return named(summary['branch']) || also.some((a) => named(a?.branch));
}
