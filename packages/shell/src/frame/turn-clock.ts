// When a turn started, as the ledger and the btw tips print it (v1 core/format.js turnTime: SHORT_TIME through
// `Intl.DateTimeFormat([], …)`): respects Settings → Chat → Time format (`locale` | `12-hour` | `24-hour`).
// Pure: the locale and format come in for tests.

import type { TimeFormat } from './chat-settings.ts';

const CLOCK_LOCALE = { hour: '2-digit', minute: '2-digit' } as const;
const CLOCK_12 = { hour: 'numeric', minute: '2-digit', hour12: true } as const;
const CLOCK_24 = { hour: '2-digit', minute: '2-digit', hour12: false } as const;

const FMT_LOCALE = new Intl.DateTimeFormat(undefined, CLOCK_LOCALE);
const FMT_12 = new Intl.DateTimeFormat(undefined, CLOCK_12);
const FMT_24 = new Intl.DateTimeFormat(undefined, CLOCK_24);
const DAY_FMT = new Intl.DateTimeFormat('en-GB', { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' });

/** 14:05 (or 2:05 PM): the turn's clock time in `locale` and `format` (Settings → Chat → Time format). */
export function turnClock(at: number, locale?: string, format: TimeFormat = 'locale'): string {
  if (locale === undefined) {
    if (format === '12-hour') return FMT_12.format(at);
    if (format === '24-hour') return FMT_24.format(at);
    return FMT_LOCALE.format(at);
  }
  const opts = format === '12-hour' ? CLOCK_12 : format === '24-hour' ? CLOCK_24 : CLOCK_LOCALE;
  return new Intl.DateTimeFormat(locale, opts).format(at);
}

/** Hover title for a turn timestamp, respecting Settings → Chat → Time format. */
export function turnLongTime(at: number, format: TimeFormat = 'locale'): string {
  return `${DAY_FMT.format(at)}, ${turnClock(at, undefined, format)}`;
}
