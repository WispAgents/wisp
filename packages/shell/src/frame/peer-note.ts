// What the bar over the list says about one peer (v1 reconnecting.js, ADR-0065 §4), pure so it's tested without a
// DOM: offline with when it was last heard from and Retry (reconnect now, skipping the backoff), asleep with Wake,
// too old with the command that updates it. Rows from the last cursor stay on screen under it either way.

import '../i18n/lazy.ts';
import type { PeerStatus } from '@wisp/client';

export interface PeerNote {
  readonly tone: 'info' | 'warn' | 'bad';
  readonly text: string;
  readonly action?: 'wake' | 'retry';
  /** the command that fixes it, shown as code (`npm install -g wisp-agents@latest`) */
  readonly command?: string;
}

type T = (key: string, vars?: Record<string, string | number>) => string;

/** 12:04 (or 12:04 PM): when it was heard from today; the date as well before today. In `locale`, the app's own when
 * left out (as the turn clock and the ledger: never a forced en-GB). */
export function heardLabel(at: number, now: number, locale?: string): string {
  const sameDay = new Date(at).toDateString() === new Date(now).toDateString();
  const clock = new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(at);
  return sameDay ? clock : `${new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' }).format(at)} ${clock}`;
}

/** `locale`: tests only; the app's own when left out. */
export function peerNote(s: PeerStatus, name: string, t: T, now: number, locale?: string): PeerNote | undefined {
  switch (s.kind) {
    case 'online':
      return undefined;
    case 'connecting':
      return { tone: 'info', text: t('shell.peer.connecting', { name }) };
    case 'asleep':
      return { tone: 'info', text: t('shell.peer.asleep', { name }), action: 'wake' };
    case 'waking':
      return { tone: 'info', text: t('shell.peer.waking', { name }) };
    case 'unreachable':
      return {
        tone: 'warn',
        text: s.heardAt === undefined ? t('shell.peer.offline', { name }) : t('shell.peer.offline-since', { name, at: heardLabel(s.heardAt, now, locale) }),
        action: 'retry',
      };
    case 'incompatible':
      return { tone: 'bad', text: t(s.update === 'app' ? 'shell.peer.updateApp' : 'shell.peer.update', { name }), ...(s.command !== undefined && { command: s.command }) };
  }
}
