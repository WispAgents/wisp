// The crew on phones as stories (ADR-0065 step 13b, docs/design/crew-stories.html C): what each ring says, in what
// order the rings go, and what a member's story shows and where its reply goes. Pure, over the crew's own data
// (data.ts: members and their jobs from the `crew` and `list` topics), so the tests read the same rules the row does.
import { cadenceText, type State, type T } from './model.ts';
import type { Member, SessionRow } from './types.ts';
import './i18n/lazy.ts';

/** A ring: waiting on you (an open ask or approval), a report you haven't read, working, idle. */
export type Ring = 'wait' | 'report' | 'work' | 'idle';

export const ringOf = (st: State): Ring => (st.asks ? 'wait' : st.unread > 0 ? 'report' : st.live.length > 0 ? 'work' : 'idle');

const RANK: Readonly<Record<Ring, number>> = { wait: 0, report: 1, work: 2, idle: 3 };

/** Waiting on you first, then an unread report, then working, then idle; within one, the crew's own rank (rankCrew). */
export function storyOrder<M extends { readonly id: string }>(ranked: readonly M[], states: ReadonlyMap<string, State>): M[] {
  const ring = (m: M): number => {
    const st = states.get(m.id);
    return st ? RANK[ringOf(st)] : RANK.idle;
  };
  return ranked
    .map((m, i) => ({ m, i, r: ring(m) }))
    .sort((a, b) => a.r - b.r || a.i - b.i)
    .map((x) => x.m);
}

/** The story `by` steps from `at` lands on, or `at` at either end. */
export const stepStory = (at: number, by: number, count: number): number => Math.min(Math.max(at + by, 0), Math.max(count - 1, 0));

const latest = (jobs: readonly SessionRow[]): SessionRow | undefined => [...jobs].sort((a, b) => b.updatedAt - a.updatedAt)[0];

/** The job asking you (the newest), its answer controls in the story's Waiting for you. */
export const askJob = (st: State): SessionRow | undefined => latest(st.jobs.filter((s) => s.state === 'waiting'));

/** What it's on now: a running job (started before one still starting), else the one asking you. */
export const nowJob = (st: State): SessionRow | undefined => st.live.find((s) => !s.pending) ?? st.live[0] ?? askJob(st);

/** Its last report: the newest job that came back (an unread one first). */
export function lastReport(st: State): SessionRow | undefined {
  const back = st.jobs.filter((s) => !s.pending && (s.state === 'review' || s.state === 'done'));
  return latest(back.filter((s) => s.unread && s.state === 'review')) ?? latest(back);
}

/** Where the reply box sends: a follow-up on the job it's running (or the one asking you), else a new job. */
export type ReplyTarget = { readonly kind: 'follow'; readonly job: SessionRow } | { readonly kind: 'assign' };

export function replyTarget(st: State): ReplyTarget {
  const job = st.live.find((s) => !s.pending && s.state === 'running') ?? askJob(st);
  return job ? { kind: 'follow', job } : { kind: 'assign' };
}

/** The Next card: when it looks again, else its schedule in words; paused says so. */
export function nextLine(m: Pick<Member, 'cron' | 'on' | 'paused' | 'wakeAt'>, c: { readonly t: T; readonly now: number; readonly when: (at: number) => string }): string {
  if (m.paused) return c.t('wisp.crew.status.paused');
  if (m.wakeAt && m.wakeAt > c.now) return c.t('wisp.crew.stories.checkinAt', { when: c.when(m.wakeAt) });
  return cadenceText(m, c.t);
}

/** The ring's words for a screen reader: "Ada, waiting on you". */
export const ringLabel = (name: string, ring: Ring, t: T): string => t(`wisp.crew.stories.ring.${ring}`, { name });
