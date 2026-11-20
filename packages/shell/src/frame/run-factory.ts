// A factory run's factory (ADR-0114), apart from reply-hint.ts so Home's first screen never carries it: only the task
// view reads it.

/** A factory run's factory, from its row's summary (`factory`, wisp.factory's line, ADR-0114): its task names the factory
 *  ("Factory · careful") where another task names its harness. A station's own task isn't a run. */
export function runFactory(summary: { readonly factory?: unknown } | undefined): string | undefined {
  const f = summary?.factory as { name?: unknown } | undefined;
  if (!f || typeof f !== 'object') return undefined;
  return typeof f.name === 'string' && f.name ? f.name : 'default';
}

/** A factory run's reply box (ADR-0114 rule 14): no harness or model chip (no agent of its own takes a follow-up, and
 *  the run refuses one), the stations named as what does the work, and Stop while the run still goes. */
export function factoryReply(summary: { readonly factory?: unknown } | undefined): { readonly name: string; readonly live: boolean } | undefined {
  const name = runFactory(summary);
  if (name === undefined) return undefined;
  const state = (summary?.factory as { state?: unknown }).state;
  return { name, live: state === undefined || state === 'running' || state === 'waiting' };
}
