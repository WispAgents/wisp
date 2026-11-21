// Whether a task is held before it starts (Ask near the limit, a changed harness, project trust, a failed setup): the
// one rule the list's pill, Home's Needs you and the task header all read (TRY-17 #28: two of them said Finished).
// `summary`: the runtime's summary, whose `status` is `held` and `held` the held turn's hold (runtime core summary.rs).

export function isHeld(state: string, summary: { readonly status?: unknown; readonly held?: unknown } | undefined): boolean {
  if (state !== 'review' || !summary) return false;
  return summary.status === 'held' || (typeof summary.held === 'object' && summary.held !== null);
}
