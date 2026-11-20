// A factory run's factory (ADR-0114), apart from reply-hint.ts so Home's first screen never carries it: only the task
// view reads it.

/** A factory run's factory, from its row's summary (`factory`, wisp.factory's line, ADR-0114): its task names the factory
 *  ("Factory · careful") where another task names its harness. A station's own task isn't a run. */
export function runFactory(summary: { readonly factory?: unknown } | undefined): string | undefined {
  const f = summary?.factory as { name?: unknown } | undefined;
  if (!f || typeof f !== 'object') return undefined;
  return typeof f.name === 'string' && f.name ? f.name : 'default';
}
