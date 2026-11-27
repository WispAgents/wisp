// A member's page (v1 crew.js buildView/paint), DOM-free: the facts down its side, the thread's items as
// cards or quiet lines, and what each card says. The component (CrewPage.tsx) only draws these.
import { cadenceLabel, eventsLabel, threadItems, type ThreadItem } from './model.ts';
import type { Key } from './i18n.ts';
import type { Member, SessionRow } from './types.ts';
import './i18n/lazy.ts';

type T = (k: Key, vars?: Readonly<Record<string, string | number>>) => string;

export interface Fact {
  readonly icon: 'clock' | 'queue' | 'folder' | 'agent' | 'replay' | 'crew';
  readonly text: string;
  readonly logo?: string | undefined;
  /** the fact opens something (the delegates: Org on them, v1's › row) */
  readonly onClick?: (() => void) | undefined;
}

const base = (p: string): string => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

/** v1's side: cadence, how many at once, where (and their other folders), which harness and mode, their fallbacks, delegates. */
export function facts(m: Member, o: { t: T; harness: (id: string) => { name: string; logo?: string | undefined }; place: (machine: string) => string; onTeam?: () => void }): Fact[] {
  const { t } = o;
  const h = o.harness(m.agent);
  const mode = m.mode === 'plan' || m.mode === 'edits' ? m.mode : 'auto';
  const team = m.team?.length ?? 0;
  return [
    // v1's side: the schedule in words ("Only when asked"), then what else wakes them
    { icon: 'clock', text: [cadenceLabel(m.cron, t), eventsLabel(m, t)].filter(Boolean).join(' · ') },
    { icon: 'queue', text: t('wisp.crew.page.parallel', { count: Math.max(1, m.parallel || 1) }) },
    {
      icon: 'folder',
      text: [
        m.cwd ? base(m.cwd) : t('wisp.crew.page.noFolder'),
        ...(m.cwd && m.dirs?.length ? [t('wisp.crew.page.moreFolders', { count: m.dirs.length })] : []),
        m.machine ? o.place(m.machine) : t('wisp.crew.page.here'),
      ].join(' · '),
    },
    { icon: 'agent', text: [h.name, t(`wisp.crew.page.mode.${mode}`)].join(' · '), logo: h.logo },
    // where a job carries on at a usage limit, in order
    ...(m.fallbacks?.length
      ? [{ icon: 'replay' as const, text: t('wisp.crew.page.fallbacks', { list: m.fallbacks.map((f) => [f.agent ? o.harness(f.agent).name : h.name, f.model].filter(Boolean).join(' ')).join(', ') }) }]
      : []),
    { icon: 'crew', text: team ? t('wisp.crew.page.delegates', { count: team }) : t('wisp.crew.page.delegates.none'), onClick: o.onTeam },
  ];
}

/** A quiet run's line: check-ins (or jobs) that found nothing. */
export function quietLabel(jobs: readonly SessionRow[], t: T): string {
  const n = jobs.length;
  return jobs.every((s) => s.crew?.job === 'checkin') ? t('wisp.crew.page.quiet', { count: n }) : t('wisp.crew.page.quietJobs', { count: n });
}

/** A card's pill: asking you, working, or how it ended. */
export function pillOf(s: SessionRow, t: T): { label: string; tone: string } | undefined {
  if (s.state === 'waiting') return { label: t('wisp.crew.page.pill.asks'), tone: 'warn' };
  if (s.state === 'running') return { label: t('wisp.crew.page.pill.working'), tone: 'info' };
  if (s.state === 'review' || s.state === 'done') return { label: t('wisp.crew.page.pill.done'), tone: s.status === 'failed' ? 'bad' : 'good' };
  return undefined;
}

/** The thread: the newest `show` items, how many earlier ones there are. */
export const thread = (jobs: readonly SessionRow[], show: number): { items: ThreadItem[]; earlier: number } => threadItems(jobs, show);
