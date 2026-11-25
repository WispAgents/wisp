// untilLabel, apart from time-format.ts (which the first screen loads) since only lazy code shows a wait (ADR-0011).
const MIN = 60_000;

/** How long until something (a plan window's reset): "45m", "2h 10m", "3d 4h". The one duration both the runner
 *  picker and Usage show (time-format.ts `timeLabel` gives the clock time a future moment falls on; this gives the wait). */
export function untilLabel(ms: number): string {
  const m = Math.max(1, Math.round(ms / MIN));
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${m % 60}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}
