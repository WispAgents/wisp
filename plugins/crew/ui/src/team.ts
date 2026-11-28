// The lead's team (v1 team.js teamLiveHtml / renderSubs, F-crew-11): the crew jobs a task handed out (delegated,
// tagged, or asked from its reply box with `@Ada`: a job whose `from` is this task), one row each in the order they went, each opening the job (a task of its own). Pure: the rows
// from the crew's jobs and members; the pane lays them out and opens them.
import type { SessionRow } from './types.ts';
import './i18n/lazy.ts';

export type Say = (k: string, v?: Readonly<Record<string, string | number>>) => string;

export interface TeamMember {
  readonly id: string;
  readonly name: string;
  readonly avatar?: string;
  readonly color?: string;
}

export interface TeamRow {
  /** the job's task id */
  readonly id: string;
  readonly member: string;
  readonly name: string;
  readonly avatar: string;
  readonly color: string;
  /** "@Ada" */
  readonly handle: string;
  /** what it is doing (live), else what it came back with, else what it was asked */
  readonly line: string;
  readonly status: string;
  readonly tone: 'accent' | 'warn' | 'good' | 'bad' | 'neutral';
  readonly live: boolean;
  /** its job is still being made (crew.assign's optimistic row): nothing to open yet */
  readonly pending: boolean;
  /** where it runs (v1 team.js ranOn): this computer, or the machine it runs on */
  readonly place: TeamPlace;
  /** its run is still going: working, or asking you (its time ticks) */
  readonly running: boolean;
  /** when its last run started and ended, for its time (v1 .elapsed while it runs, .sub-dur once it's back) */
  readonly startedAt?: number;
  readonly endedAt?: number;
}

export interface TeamPlace {
  /** "This computer", or the machine's name */
  readonly name: string;
  readonly icon: 'laptop' | 'server';
  /** on another machine than its lead */
  readonly away: boolean;
  /** v1's tooltip: "Ran on Studio, in Ada's own folder" */
  readonly tip: string;
}

const prefix = (k: string): string => (k.includes(':') ? k.slice(0, k.indexOf(':')) : '');

/** Where job `s` of the lead `lead` runs, seen from peer `here` (v1 team.js ranOn: a crew job runs in the member's own
 *  folder, on their machine when they have one). A job's row comes from the peer it runs on (a member's machine is a
 *  peer of its own, `s.peer`); the lead's is its key's prefix. */
export function placeOf(s: SessionRow, lead: string, here: string, who: string, say: Say, hereName?: string): TeamPlace {
  const leadPeer = prefix(lead) || here;
  const peer = s.peer || leadPeer;
  const name = peer === here ? (hereName ?? say('wisp.crew.page.here')) : s.machine || say('wisp.crew.team.elsewhere');
  const away = peer !== leadPeer;
  // v1: a server when it's away from its lead or on a machine; a laptop only here, with a lead that's here too
  return { name, icon: away || peer !== here ? 'server' : 'laptop', away, tip: say('wisp.crew.team.ranOn', { where: name, name: who }) };
}

/** What the row's time says, in ms (v1 team.js): a job whose run is still going (working, or asking you: v1 counts its
 *  turn as running) ticks from its run's start (`now − startedAt`), one that's back says how long it took (`endedAt −
 *  startedAt`); one still being made, or with no run yet, none. */
export function timeOf(r: Pick<TeamRow, 'running' | 'pending' | 'startedAt' | 'endedAt'>, now: number): number | undefined {
  if (r.pending || r.startedAt === undefined) return undefined;
  if (r.running) return Math.max(0, now - r.startedAt);
  return r.endedAt !== undefined && r.endedAt >= r.startedAt ? r.endedAt - r.startedAt : undefined;
}

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
/** the lead a job came from: the task key's id or the key itself */
const fromLead = (s: SessionRow, lead: string): boolean => {
  const f = s.crew?.from?.id;
  return !!f && (f === lead || f === bare(lead) || bare(f) === bare(lead));
};

// a job you sent from the member's page has no `from`; a check-in or a trigger's isn't this task's work
const handedOut = (s: SessionRow, lead: string): boolean => (s.crew?.job === 'delegated' || s.crew?.job === 'mention' || s.crew?.job === 'task') && fromLead(s, lead);

/** Whether task `lead` handed any crew job out (its Team tab shows). */
export const hasTeam = (jobs: readonly SessionRow[], lead: string): boolean => jobs.some((s) => handedOut(s, lead));

function statusOf(s: SessionRow, say: Say): Pick<TeamRow, 'status' | 'tone' | 'live'> {
  if (s.pending) return { status: say('wisp.crew.job.starting'), tone: 'accent', live: true };
  if (s.state === 'running') return { status: say('wisp.crew.job.working'), tone: 'accent', live: true };
  if (s.state === 'waiting') return { status: say('wisp.crew.job.asking'), tone: 'warn', live: false };
  if (s.status === 'error' || s.status === 'failed' || s.status === 'blocked') return { status: say('wisp.crew.team.failed'), tone: 'bad', live: false };
  if (s.status === 'stopped') return { status: say('wisp.crew.team.stopped'), tone: 'neutral', live: false };
  return { status: say('wisp.crew.team.done'), tone: 'good', live: false };
}

/** The jobs task `lead` (its entity key or id) handed to crew members, oldest first. `here`: this wisp's peer id. */
/** `hereName`: what this wisp is called when it isn't This computer (a team's hub, ADR-0148). */
export function teamRows(jobs: readonly SessionRow[], lead: string, memberOf: (id: string) => TeamMember | undefined, say: Say, here = '', hereName?: string): TeamRow[] {
  return jobs
    .filter((s) => handedOut(s, lead))
    .slice()
    .sort((a, b) => a.createdAt - b.createdAt || a.id.localeCompare(b.id))
    .map((s) => {
      const m = memberOf(s.crew?.id ?? '');
      const name = m?.name ?? '';
      const st = statusOf(s, say);
      const line = (st.live ? s.live : s.preview) || s.title || s.crew?.prompt || '';
      const handle = name ? `@${name}` : say('wisp.crew.team.someone');
      const place = placeOf(s, lead, here, name || handle, say, hereName);
      return {
        id: s.id,
        member: s.crew?.id ?? '',
        name,
        avatar: m?.avatar ?? '',
        color: m?.color ?? '',
        handle,
        line,
        ...st,
        pending: !!s.pending,
        running: s.state === 'running' || s.state === 'waiting',
        place,
        ...(s.startedAt !== undefined && { startedAt: s.startedAt }),
        ...(s.endedAt !== undefined && { endedAt: s.endedAt }),
      };
    });
}

/** A long team folds (v1 c33f33e6 team.js shownOf): this many rows show, the rest wait behind Show N more. */
export const TEAM_SHOWN = 3;

/** Which rows stay shown, by index, in their order: the running ones first (the newest of them), then the newest
 *  that are back. Undefined: every row shows (folding a single row isn't worth a button). */
export function shownOf(rows: readonly { readonly running: boolean }[], n = TEAM_SHOWN): ReadonlySet<number> | undefined {
  if (rows.length <= n + 1) return undefined;
  const rank = rows.map((_, i) => i).sort((a, b) => Number(rows[b]?.running) - Number(rows[a]?.running) || b - a);
  return new Set(rank.slice(0, n));
}
