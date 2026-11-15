// When a turn started, as the ledger and the btw tips print it (v1 core/format.js turnTime: SHORT_TIME through
// `Intl.DateTimeFormat([], …)`): the app's locale, like the kit's <Time> (content/time-format.ts), so a 12-hour
// locale reads "2:05 PM" and a 24-hour one "14:05". Pure: the locale comes in for tests.

const CLOCK = { hour: '2-digit', minute: '2-digit' } as const;
const DEFAULT = new Intl.DateTimeFormat(undefined, CLOCK);

/** 14:05 (or 02:05 PM): the turn's clock time in `locale`, the app's own when left out. */
export function turnClock(at: number, locale?: string): string {
  return (locale === undefined ? DEFAULT : new Intl.DateTimeFormat(locale, CLOCK)).format(at);
}
