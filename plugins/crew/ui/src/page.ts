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

// ---------- the hero (ADR-0142) ----------

const DAY = 86_400_000;

/** How long a run took, short: "40s", "14m", "2h 5m". */
export function shortDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h}h ${m % 60}m` : `${h}h`;
}

/** The hero's numbers, all from its jobs and its delegates: jobs done, this week, how long a job takes, who it hands
 * work to. A number with nothing behind it is left out (a new member shows none). */
export function heroStats(m: Pick<Member, 'team'>, jobs: readonly SessionRow[], o: { t: T; now: number }): { value: string; label: string }[] {
  const { t } = o;
  const done = jobs.filter((s) => (s.state === 'review' || s.state === 'done') && s.status !== 'failed').length;
  const week = jobs.filter((s) => s.createdAt > o.now - 7 * DAY).length;
  const runs = jobs.flatMap((s) => (s.startedAt && s.endedAt && s.endedAt > s.startedAt ? [s.endedAt - s.startedAt] : []));
  const team = m.team ?? [];
  const out: { value: string; label: string }[] = [];
  if (done)
    out.push({
      value: String(done),
      label: t('wisp.crew.page.stat.done', { count: done }),
    });
  if (week) out.push({ value: String(week), label: t('wisp.crew.page.stat.week') });
  if (runs.length)
    out.push({
      value: shortDuration(runs.reduce((a, b) => a + b, 0) / runs.length),
      label: t('wisp.crew.page.stat.avg'),
    });
  if (team.length === 1)
    out.push({
      value: team[0]?.label || team[0]?.name || '',
      label: t('wisp.crew.page.stat.handsTo'),
    });
  else if (team.length > 1)
    out.push({
      value: t('wisp.crew.page.stat.helpers', { count: team.length }),
      label: t('wisp.crew.page.stat.handsTo'),
    });
  return out;
}

/** Above the box: the jobs you gave it, newest first, each once (at most `max`). */
export function againPrompts(jobs: readonly SessionRow[], max = 3): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const s of [...jobs].sort((a, b) => b.createdAt - a.createdAt)) {
    const p = s.crew?.job === 'task' ? s.crew.prompt?.trim() : undefined;
    if (!p || seen.has(p)) continue;
    seen.add(p);
    out.push(p);
    if (out.length >= max) break;
  }
  return out;
}

/** The hero's folder chip: the folder it starts in (and how many more), and the computer when it isn't this one. */
export function folderChip(m: Pick<Member, 'cwd' | 'dirs' | 'machine'>, o: { t: T; place: (machine: string) => string }): string {
  return [
    m.cwd ? base(m.cwd) : o.t('wisp.crew.page.noFolderChip'),
    ...(m.cwd && m.dirs?.length ? [o.t('wisp.crew.page.moreFolders', { count: m.dirs.length })] : []),
    ...(m.machine ? [o.place(m.machine)] : []),
  ].join(' · ');
}
