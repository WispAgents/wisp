// The bar over the list for a peer that isn't online (ADR-0065 §4): offline says when it was last heard from, with
// Retry; a wisp too old says Update wisp on <name> with the command.
import { expect, test } from 'vitest';
import { heardLabel, peerNote } from '../src/frame/peer-note.ts';
import en from '../src/i18n/en.lazy.json' with { type: 'json' };

const t = (key: string, vars: Record<string, string | number> = {}): string => ((en as Record<string, string>)[key] ?? key).replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? ''));
const at = (h: number, m: number): number => new Date(2026, 10, 15, h, m).getTime();

test('offline_says_when_it_was_last_updated_with_retry', () => {
  expect(peerNote({ kind: 'unreachable', reason: 'no network', heardAt: at(12, 4) }, 'Studio', t, at(12, 30), 'en-GB')).toEqual({ tone: 'warn', text: 'Studio is offline · last updated 12:04', action: 'retry' });
  // never heard from (a cold start with no saved stream): just offline, Retry all the same
  expect(peerNote({ kind: 'unreachable', reason: 'x' }, 'Studio', t, at(12, 30))).toEqual({ tone: 'warn', text: 'Studio is offline', action: 'retry' });
  // yesterday's: the date too, so 12:04 isn't read as today
  expect(peerNote({ kind: 'unreachable', reason: 'x', heardAt: at(12, 4) - 86_400_000 }, 'Studio', t, at(12, 30), 'en-GB')?.text).toBe('Studio is offline · last updated 14 Nov 12:04');
  expect(peerNote({ kind: 'online' }, 'Studio', t, 0)).toBeUndefined();
  expect(peerNote({ kind: 'asleep' }, 'Box', t, 0)?.action).toBe('wake');
});

test('an_older_wisp_says_update_with_the_command', () => {
  expect(peerNote({ kind: 'incompatible', reason: 'Update wisp on Studio', update: 'peer', command: 'npm install -g wisp-agents@latest' }, 'Studio', t, 0)).toEqual({ tone: 'bad', text: 'Update wisp on Studio', command: 'npm install -g wisp-agents@latest' });
  expect(peerNote({ kind: 'incompatible', reason: 'x', update: 'app' }, 'Studio', t, 0)).toEqual({ tone: 'bad', text: 'Update the app to reach Studio' });
});

test('offline_since_uses_the_app_locale_not_en_gb', () => {
  // as the turn clock and the ledger (f838e783, c659fbee): a 12-hour locale reads 2:05 PM, a 24-hour one 14:05
  expect(heardLabel(at(14, 5), at(14, 30), 'en-US')).toBe('02:05 PM');
  expect(heardLabel(at(14, 5), at(14, 30), 'en-GB')).toBe('14:05');
  expect(heardLabel(at(14, 5) - 86_400_000, at(14, 30), 'en-US')).toBe('Nov 14 02:05 PM');
  // the app's own locale when none is given: what Intl picks, never a forced en-GB
  expect(heardLabel(at(14, 5), at(14, 30))).toBe(new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(at(14, 5)));
});
