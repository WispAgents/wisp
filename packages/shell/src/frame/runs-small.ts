// The few Compare-runs helpers the first page uses (Home's vs +, the dialog's params, a plan window's name), apart from
// compare.ts: a module both the entry and a lazy chunk import stays whole in the entry (ADR-0067), and compare.ts's
// table and Plan-row math is only the Compare dialog's. compare.ts re-exports them.

/** the dialog's params: the task it was opened from */
export const parseCompare = (p: unknown): { id: string } => ({ id: String((p as { id?: unknown } | null)?.id ?? '') });

/** v1 runners.js MAX_RUNNERS: the lead and up to three more side by side */
export const MAX_RUNNERS = 4;

/**
 * The launcher's vs + (v1 launcher.js vsBtn): the next harness nobody in the pill uses yet, among those that can run
 * here; once they're all in, another copy of the lead. None past MAX_RUNNERS.
 */
export function nextRun(lead: string, runs: readonly string[], available: readonly string[]): string | undefined {
  if (runs.length >= MAX_RUNNERS - 1) return undefined;
  const used = new Set([lead, ...runs]);
  return available.find((a) => a !== 'auto' && !used.has(a)) ?? (lead === 'auto' ? available.find((a) => a !== 'auto') : lead);
}

/** A group id for the runs of one launch (v1: 6 random bytes in hex). */
export const newGroup = (bytes: Uint8Array): string => `g-${[...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')}`;

const WINDOW_NAMES: Readonly<Record<string, string>> = { five_hour: '5-hour', seven_day: 'weekly', seven_day_opus: 'weekly Opus', seven_day_sonnet: 'weekly Sonnet' };

/** "5-hour", "weekly", "3-day" (v1 receipt.js windowName) */
export function windowName(id: string): string {
  const named = WINDOW_NAMES[id];
  if (named) return named;
  const mins = Number(/^w(\d+)m$/.exec(id)?.[1]) || 0;
  return !mins ? 'plan' : mins >= 1440 ? `${Math.round(mins / 1440)}-day` : `${Math.round(mins / 60)}-hour`;
}
