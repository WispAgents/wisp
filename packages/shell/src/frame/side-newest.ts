// UX 2026-11-18 desktop #5: the side panel opens on the newest thing the task has: its PR once there is one (wisp.ship
// keeps it on the row: `pr`, `prs`, a compare run's `also`), else Changes when the work changed files, else Shell.
// DOM-free.
type Row = { readonly summary?: Record<string, unknown> } | undefined;

export function newestSide(row: Row, changedFiles: number): 'pr' | 'changes' | 'shell' {
  const s = (row?.summary ?? {}) as { pr?: unknown; prs?: readonly unknown[]; also?: readonly { pr?: unknown }[]; toShip?: { dirty?: number; ahead?: number } | null };
  if (s.pr || (s.prs ?? []).length > 0 || (s.also ?? []).some((a) => !!a?.pr)) return 'pr';
  if (changedFiles > 0 || (s.toShip?.dirty ?? 0) > 0 || (s.toShip?.ahead ?? 0) > 0) return 'changes';
  return 'shell';
}
