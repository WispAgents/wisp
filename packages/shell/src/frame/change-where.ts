// What the task head's Changes +n −n reads (git's diff of the task's worktree), as a resource's source: undefined while
// the task has no worktree, so the read never runs. Audit F setup F2: it was '' then, which Solid's createResource
// fetches (it skips only undefined, null and false): the fetcher ran JSON.parse('') and the resource held that error;
// when the worktree appeared, the head read the stale error and the page crashed ("Unexpected end of JSON input").

type Wt = { path?: string; cwd?: string; baseRef?: string; base?: string };
interface Row {
  readonly body?: unknown;
  readonly summary: Record<string, unknown>;
}

/** The read's key (the folder, its base, the last turn and what's left to ship), or undefined: no worktree, nothing to read. */
export function changeWhere(r: Row | undefined, last: { readonly id?: string; readonly status?: string } | undefined): string | undefined {
  if (!r) return undefined;
  const body = (r.body ?? {}) as { ext?: Record<string, Wt | undefined> };
  const wt = body.ext?.['wisp.worktrees'] ?? (r.summary['worktree'] as Wt | undefined);
  // the worktree only, as the side panel's Changes (v1 GET /sessions/:id/changes): a task in place diffed the project
  // folder, so its head said +1962 for every session's and the user's own work (side-panel audit, 2026-11-19)
  const dir = String(wt?.path ?? wt?.cwd ?? '');
  if (!dir) return undefined;
  return JSON.stringify([dir, wt?.baseRef || wt?.base || 'HEAD', last?.id ?? '', last?.status ?? '', r.summary['toShip'] ?? null]);
}

/** The folder and base a key names (a key that isn't one: none). */
export function whereOf(w: string): { readonly path: string; readonly base: string } | undefined {
  try {
    const [path, base] = JSON.parse(w) as [unknown, unknown];
    return typeof path === 'string' && path && typeof base === 'string' ? { path, base } : undefined;
  } catch {
    return undefined;
  }
}
