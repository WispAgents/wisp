// The Crew tab in the phone apps (ADR-0065 step 14, design/mobile.md §A.2 Crew), DOM-free: the members list's rows
// (most-needed first, as the stories row ranks them), the member page's three tabs (Jobs · Brief · Wakes), each job's
// row, and the menus (the root bar's ⋮ / ⋯ and a member's). The screens (CrewPhone.tsx) only draw these, so every v1
// crew feature (docs/v1-features.md F-crew) is a named entry here and tested without a DOM.
import type { MenuEntry } from '@wisp/ui';
import { cadenceLabel, eventsLabel, JOB_ICON, originLabel, statusLine, type State } from './model.ts';
import { pillOf } from './page.ts';
import { nextLine } from './stories.ts';
import type { Key } from './i18n.ts';
import type { Member, SessionRow } from './types.ts';
import './i18n/lazy.ts';

type T = (k: Key, vars?: Readonly<Record<string, string | number>>) => string;
type Ctx = {
  readonly t: T;
  readonly now: number;
  readonly when: (at: number) => string;
};

/** The member page's tabs: M3 primary tabs on Android, a segmented control on iOS. */
export const TABS = ['jobs', 'brief', 'wakes'] as const;
export type PhoneTab = (typeof TABS)[number];
export const isTab = (v: string): v is PhoneTab => (TABS as readonly string[]).includes(v);

export interface MemberRow {
  readonly id: string;
  readonly name: string;
  /** role · what it's on (asks, works, reported, its cadence) */
  readonly sub: string;
  /** what it's on alone (the member page's head, under its role) */
  readonly line: string;
  readonly tone: 'asks' | 'live' | 'report' | 'idle' | 'paused' | string;
  /** a word for the pill: Asks you / Working / New report; none when idle */
  readonly pill?: { readonly label: string; readonly tone: 'warn' | 'info' | 'accent' } | undefined;
}

/** One two-line row per member: its name, then its role and what it's doing (v1's rail card in words). */
export function memberRow(m: Member, st: State, c: Ctx): MemberRow {
  const line = statusLine(m, st, { t: c.t as never, now: c.now, when: c.when });
  const sub = [m.role, line.text].filter(Boolean).join(' · ');
  const pill =
    line.tone === 'asks'
      ? ({ label: c.t('wisp.crew.page.pill.asks'), tone: 'warn' } as const)
      : line.tone === 'live'
        ? ({ label: c.t('wisp.crew.page.pill.working'), tone: 'info' } as const)
        : line.tone === 'report'
          ? ({
              label: c.t('wisp.crew.phone.pill.report'),
              tone: 'accent',
            } as const)
          : undefined;
  return {
    id: m.id,
    name: m.name,
    sub,
    line: line.text,
    tone: line.tone,
    pill,
  };
}

export interface JobRow {
  readonly id: string;
  readonly title: string;
  /** what it asks, what it's doing now, or what it found */
  readonly sub: string;
  readonly icon: (typeof JOB_ICON)[keyof typeof JOB_ICON];
  /** where it came from (You, Check-in, Delegated by …) */
  readonly origin: string;
  readonly pill?: { readonly label: string; readonly tone: string } | undefined;
  readonly at: number;
  readonly unread: boolean;
}

/** A job's row on the Jobs tab, newest first (the browser's thread, oldest first, reads down a wide pane). */
export function jobRows(jobs: readonly SessionRow[], c: Pick<Ctx, 't'>): JobRow[] {
  return [...jobs]
    .sort((a, b) => b.createdAt - a.createdAt)
    .map((s) => ({
      id: s.id,
      title: s.title || s.crew?.prompt || c.t('wisp.crew.phone.untitled'),
      sub: s.state === 'waiting' ? s.question || s.live || '' : s.state === 'running' ? s.live || c.t('wisp.crew.page.starting') : s.preview || '',
      icon: JOB_ICON[s.crew?.job ?? 'task'],
      origin: originLabel(s, c.t as never),
      pill: pillOf(s, c.t as never),
      at: s.createdAt,
      unread: !!s.unread && s.state === 'review',
    }));
}

export interface WakeLine {
  readonly id: 'cadence' | 'events' | 'next' | 'checkins';
  readonly title: string;
  readonly sub: string;
}

/** The Wakes tab: when it checks in on its own, what else starts it (F-crew-7), the next check-in and how many so far. */
export function wakeLines(m: Member, c: Ctx): WakeLine[] {
  const events = eventsLabel(m, c.t as never);
  return [
    {
      id: 'cadence',
      title: c.t('wisp.crew.phone.wakes.cadence'),
      sub: m.cron ? cadenceLabel(m.cron, c.t as never) : c.t('wisp.crew.cadence.oncall'),
    },
    {
      id: 'events',
      title: c.t('wisp.crew.phone.wakes.events'),
      sub: events || c.t('wisp.crew.phone.wakes.noEvents'),
    },
    {
      id: 'next',
      title: c.t('wisp.crew.phone.wakes.next'),
      sub: nextLine(m, c),
    },
    {
      id: 'checkins',
      title: c.t('wisp.crew.phone.wakes.count'),
      sub: c.t('wisp.crew.phone.wakes.checkins', { count: m.checkins || 0 }),
    },
  ];
}

/** The Crew root's ⋮ / ⋯: what the browser's rail and Org page hold that isn't a bar button. */
export function rootMenu(c: Pick<Ctx, 't'>, has: boolean): MenuEntry[] {
  return [
    { value: 'import', label: c.t('wisp.crew.phone.import'), icon: 'download' },
    ...(has
      ? ([
          {
            value: 'shareCrew',
            label: c.t('wisp.crew.page.shareCrew'),
            icon: 'crew',
          },
          {
            value: 'record',
            label: c.t('wisp.crew.page.record'),
            icon: 'stats',
          },
        ] as const)
      : []),
  ];
}

/** A member's ⋮ / ⋯ (the browser page's More, and its side actions). */
export function memberMenu(m: Pick<Member, 'paused' | 'name'>, c: Pick<Ctx, 't'>): MenuEntry[] {
  return [
    { value: 'edit', label: c.t('wisp.crew.page.editMember'), icon: 'pen' },
    { value: 'checkin', label: c.t('wisp.crew.page.checkin'), icon: 'clock' },
    {
      value: 'pause',
      label: c.t(m.paused ? 'wisp.crew.page.resume' : 'wisp.crew.page.pause'),
      icon: m.paused ? 'play' : 'pause',
    },
    { value: 'org', label: c.t('wisp.crew.org.title'), icon: 'org' },
    { value: 'share', label: c.t('wisp.crew.page.share'), icon: 'share' },
    {
      value: 'shareCrew',
      label: c.t('wisp.crew.page.shareCrew'),
      icon: 'crew',
    },
    { value: 'record', label: c.t('wisp.crew.page.record'), icon: 'stats' },
    { kind: 'separator' },
    {
      value: 'remove',
      label: c.t('wisp.crew.page.remove'),
      icon: 'trash',
      danger: true,
    },
  ];
}
