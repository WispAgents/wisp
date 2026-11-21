// A computer that isn't connected, as its grid answers (apps/grid edge_answer.rs `offline`): 503 with its waiting page
// in `x-wisp-offline`, a path under /_grid/ that the service worker never answers (QA round 10 Grid #2). Pure, so the
// worker and its test share it.

const WAITING = '/_grid/offline';

interface Answer {
  readonly status: number;
  readonly headers: { get(name: string): string | null };
}

/** The waiting page for the page at `url`: it opens that page again once the computer is back. */
export function waitingFor(url: string): string {
  const u = new URL(url);
  return `${WAITING}?r=${encodeURIComponent(u.pathname + u.search)}`;
}

/** The waiting page the grid named for this answer, or null: only a 503's, and only that page on this origin. */
export function offlineWaiting(r: Answer | undefined): string | null {
  const to = r?.status === 503 ? r.headers.get('x-wisp-offline') : null;
  return to !== null && (to === WAITING || to.startsWith(`${WAITING}?`)) ? to : null;
}
