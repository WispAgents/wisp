// UX 2026-11-18 desktop #5: the side panel opens on the newest thing the task has: its PR once there is one (wisp.ship
// keeps it on the row: `pr`, `prs`, a compare run's `also`), else Changes when the work changed files, else Shell.
// DOM-free.
type Row = { readonly summary?: Record<string, unknown> } | undefined;

/** The PR's number and its checks (✕ one failed, ● some running, ✓ all passed), for a pill or a tab: `#806 ✓`. The
 *  first of a compare run's or a multi-repo task's PRs; none: undefined. */
export function prBadge(row: Row): string | undefined {
  type Pr = { number?: number; checks?: { pass?: number; fail?: number; pending?: number } } | null | undefined;
  const s = (row?.summary ?? {}) as { pr?: Pr; prs?: readonly { pr?: Pr }[]; also?: readonly { pr?: Pr }[] };
  const pr = s.pr ?? s.prs?.find((x) => x?.pr)?.pr ?? s.also?.find((x) => x?.pr)?.pr;
  if (!pr?.number) return undefined;
  const c = pr.checks;
  const mark = !c ? '' : (c.fail ?? 0) > 0 ? ' ✕' : (c.pending ?? 0) > 0 ? ' ●' : (c.pass ?? 0) > 0 ? ' ✓' : '';
  return `#${pr.number}${mark}`;
}

export function newestSide(row: Row, changedFiles: number): 'pr' | 'changes' | 'shell' {
  const s = (row?.summary ?? {}) as { pr?: unknown; prs?: readonly unknown[]; also?: readonly { pr?: unknown }[]; toShip?: { dirty?: number; ahead?: number } | null };
  if (s.pr || (s.prs ?? []).length > 0 || (s.also ?? []).some((a) => !!a?.pr)) return 'pr';
  if (changedFiles > 0 || (s.toShip?.dirty ?? 0) > 0 || (s.toShip?.ahead ?? 0) > 0) return 'changes';
  return 'shell';
}
