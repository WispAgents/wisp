// Ask near the limit's row in the prompt boxes (v1 features/hold.js; ADR-0091): its "Wait until …" time, as v1's
// later.js `aheadLabel` says a time ahead (14:32 today, Mon 14:32 this week, 2 Nov 14:32 after; en-GB 24-hour like v1).

const LOCALE = 'en-GB';
const HM: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' };
const fmt = (at: number, o: Intl.DateTimeFormatOptions): string => new Date(at).toLocaleString(LOCALE, o);

export function aheadLabel(at: number, now: number): string {
  const hm = fmt(at, HM);
  if (new Date(at).toDateString() === new Date(now).toDateString()) return hm;
  if (at - now < 6 * 86_400_000) return `${fmt(at, { weekday: 'short' })} ${hm}`;
  return `${fmt(at, { day: 'numeric', month: 'short' })} ${hm}`;
}

/** Wait until the reset is offered only when the reset is known and more than a minute away (v1 `showHold`). */
export const waitable = (until: number | undefined, now: number): until is number => until !== undefined && until > now + 60_000;
