// How a session row reads in v1's list (web/js/features/feed.js): its section, the state pill, the
// harness line, the goal and overlap pills, the short time. Pure, so it's tested without a page.

import { keyOfId } from '@wisp/client';
import type { SessionRow } from '../rows.ts';
import type { ShellContext } from '../contract.ts';
import { isHeld } from '../task-held.ts';

export type SectionId = 'needs' | 'working' | 'done';
export const SECTIONS: readonly SectionId[] = ['needs', 'working', 'done'];

/** Saved for later: the server's summary says `{at, by}`, an optimistic patch the same (or `true` from an older one). */
export const isSaved = (v: unknown): boolean => v === true || (typeof v === 'object' && v !== null);

const obj = (v: unknown): Record<string, unknown> | undefined => (typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : undefined);

/** Needs you, by the list's own rules (shell-list model/sections.ts, v1 feed.js groups): finished or asking, and not
 *  archived, saved for later, waiting on a follow-up (Scheduled), a subagent's run (under its lead) or a terminal task
 *  with its terminal open. Home's Needs you, the project chips and the switcher count this, so they agree with the list. */
export function needsYou(s: SessionRow): boolean {
  if (s.archived || s.parent_id) return false;
  if (s.state !== 'review' && s.state !== 'waiting') return false;
  const sum = s.summary as Record<string, unknown>;
  // saved for later (the list's isForLater: not while a reminder came due or a turn waits to send)
  if (isSaved(sum['forLater']) && !sum['reminder'] && !sum['pending']) return false;
  // a terminal task with its terminal open, finished: being worked on (the list's inTerm)
  if (s.state === 'review' && sum['tui'] && sum['term']) return false;
  // shipped (the list's isShipping): in Shipping until its PR lands
  if (s.state === 'review' && (sum['shipping'] || obj(sum['pr'])?.['state'] === 'OPEN')) return false;
  const later = obj(obj(obj(s.body?.['ext'])?.['wisp.later']))?.['later'];
  const followUp = Array.isArray(later) && later.some((l) => !obj(l)?.['remind']);
  const scheduled = s.state === 'review' && followUp && !sum['reminder'] && !sum['held'] && !sum['pending'];
  return !scheduled;
}

/** A terminal task (the harness's own CLI in wisp; the summary's `tui`): its state stays `running` while its CLI is
 *  open, but no wisp run is working, so Done is never refused (closing its CLI goes with it, server side). */
export const isTerminalTask = (s: Pick<SessionRow, 'summary'> | undefined): boolean => !!(s?.summary as Record<string, unknown> | undefined)?.['tui'];

/** Still working, so Mark as done stays put and says so (v1 actions.js, orch #18): running or asking, unless it's a
 *  terminal task. */
export const doneRefused = (s: Pick<SessionRow, 'state' | 'summary'> | undefined): boolean => (s?.state === 'running' || s?.state === 'waiting') && !isTerminalTask(s);

/** What Undo of a Done puts back: a failed task stays failed, anything else is for review (Done was refused while it
 *  worked; a terminal task's CLI closed with the Done, so it isn't running any more). */
export const undoneState = (s: Pick<SessionRow, 'state'> | undefined): 'review' | 'error' => (s?.state === 'error' ? 'error' : 'review');

/** Asking you something: v1's `waiting` state (a `runtime.ask` arrives as `waiting` with an empty `question`), or a
 *  question or an approval the summary carries. */
export function asking(s: SessionRow): boolean {
  const sum = s.summary as Record<string, unknown>;
  return s.state === 'waiting' || !!sum['question'] || sum['approval'] === true;
}

/** Home's Needs you, as the sidebar sorts it (v1 split.js paintHome reads feed.js `groups().needs`): asking first,
 *  then newest. */
export function homeNeeds(rows: readonly SessionRow[]): SessionRow[] {
  return rows.filter(needsYou).sort((a, b) => Number(asking(b)) - Number(asking(a)) || b.updated_at - a.updated_at);
}

export function sectionOf(s: SessionRow): SectionId {
  if (s.state === 'running') return 'working';
  if (s.state === 'done') return 'done';
  return 'needs';
}

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

/** v1's list time: now, 5m, 3h, 2d. */
export function shortAgo(at: number, now: number): string {
  const d = Math.max(0, now - at);
  if (d < MIN) return 'now';
  if (d < HOUR) return `${Math.floor(d / MIN)}m`;
  if (d < DAY) return `${Math.floor(d / HOUR)}h`;
  return `${Math.floor(d / DAY)}d`;
}

// made once: a DateTimeFormat costs far more to build than to use, and every list row and turn asks for its title
// (4.6 ms of a task's opening at 4×, perf r1 #6)
const DAY_FMT = new Intl.DateTimeFormat('en-GB', { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' });
const CLOCK_FMT = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' });

/** v1's hover title for a time: Sun, 8 Nov 2026, 03:28 */
export function longTime(at: number): string {
  const d = new Date(at);
  return `${DAY_FMT.format(d)}, ${CLOCK_FMT.format(d)}`;
}

export type Tone = 'done' | 'info' | 'warn' | 'bad' | 'live';

export function statusOf(s: SessionRow, t: ShellContext['t']): { label: string; tone: Tone } {
  const st = String(s.summary.status ?? '');
  if (s.state === 'running') return { label: t('shell.status.working'), tone: 'live' };
  // a question waits for you (an ask, a stalled goal): that's what the task's state says, whatever the last turn said;
  // v1's word for it is Asking (turns.js STATE_WORD, split.js paintHome), Needs approval for an approval (the list's pill)
  if (asking(s)) return { label: t(s.summary['approval'] === true ? 'shell.status.approval' : 'shell.status.asking'), tone: 'warn' };
  if (isHeld(s.state, s.summary)) return { label: t('shell.status.held'), tone: 'warn' };
  if (st === 'info') return { label: t('shell.status.report'), tone: 'info' };
  // v1 format.js statusKey: a blocked run says why it stopped (a limit, a refusal, a crash), else Blocked, as the
  // list's pill does (B9: a run refused before it started read Failed here and Blocked on its card)
  const halt = String(s.summary['halt'] ?? '');
  if (st === 'blocked' && HALT[halt]) return { label: t(HALT[halt]), tone: halt === 'failed' ? 'bad' : 'warn' };
  if (st === 'blocked') return { label: t('shell.status.blocked'), tone: 'warn' };
  if (st === 'failed' || s.state === 'error') return { label: t('shell.status.failed'), tone: 'bad' };
  if (st === 'partial') return { label: t('shell.status.partial'), tone: 'warn' };
  if (st === 'waiting') return { label: t('shell.status.waiting'), tone: 'warn' };
  return { label: t('shell.status.finished'), tone: 'done' };
}
const HALT: Readonly<Record<string, string>> = {
  failed: 'shell.status.failed',
  limit: 'shell.status.limit',
  refused: 'shell.status.refused',
  signedOut: 'shell.status.signedOut',
};

interface GoalSummary {
  readonly text?: string;
  readonly status?: string;
  readonly round?: number;
  readonly checks?: readonly { readonly progress?: number }[];
  readonly last?: { readonly votes?: readonly { readonly name: string; readonly why: string }[] };
}

export function goalOf(s: SessionRow, t: ShellContext['t']): { n: number; progress: number; title: string; paused: boolean } | undefined {
  const g = s.summary['goal'] as GoalSummary | undefined;
  if (!g?.text) return undefined;
  const progress = g.checks?.[g.checks.length - 1]?.progress ?? 0;
  const paused = g.status === 'paused';
  const votes = (g.last?.votes ?? []).map((v) => `${v.name}: ${v.why}`).join(' · ');
  const title = `${t('shell.goal.doneWhen', { text: g.text })}\n${paused ? t('shell.goal.paused') : ''}${votes ? ` · ${votes}` : ''}`;
  return { n: g.round ?? 0, progress, title, paused };
}

export interface OverlapRow {
  readonly id: string;
  readonly files: readonly string[];
  readonly conflicts?: readonly string[];
}

export const overlapsOf = (s: SessionRow): readonly OverlapRow[] => (s.summary['overlaps'] as OverlapRow[] | undefined) ?? [];

export function overlapLine(o: OverlapRow, t: ShellContext['t']): string {
  if (o.conflicts?.length) return t('shell.overlap.conflict', { files: o.conflicts.join(', ') });
  return t('shell.overlap.clean', { files: o.files.join(', ') });
}

export function overlapOf(s: SessionRow, titleOf: (id: string) => string, t: ShellContext['t']): { n: number; title: string } | undefined {
  const all = overlapsOf(s);
  if (!all.length) return undefined;
  return { n: all.length, title: all.map((o) => `“${titleOf(o.id)}”: ${overlapLine(o, t)}`).join('\n') };
}

/** A turn's time as v1 shows it: `369ms`, `12s`, `3m 4s`, `1h 2m`. */
export function duration(ms: number): string {
  const n = Math.max(0, Math.round(ms));
  if (n < 1000) return `${n}ms`;
  const s = Math.round(n / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

/** A turn's files and lines: each edit counts its new text's lines added and its old text's lines removed. */
export function changeFigures(changes: readonly { readonly file: string; readonly edits?: readonly unknown[] }[]): { files: number; plus: number; minus: number } {
  const lines = (s: unknown): number => (typeof s === 'string' && s !== '' ? s.replace(/\n$/, '').split('\n').length : 0);
  let plus = 0;
  let minus = 0;
  for (const c of changes) {
    for (const e of c.edits ?? []) {
      const x = e as { before?: unknown; after?: unknown };
      plus += lines(x.after);
      minus += lines(x.before);
    }
  }
  return { files: new Set(changes.map((c) => c.file)).size, plus, minus };
}

/** Older turns fold away behind one line once a task has more than `keep` (v1 transcript.js): the newest stay. */
export function folded<T>(turns: readonly T[], open: boolean, keep = 3): { hidden: number; shown: readonly T[] } {
  if (open || turns.length <= keep + 1) return { hidden: 0, shown: turns };
  return { hidden: turns.length - keep, shown: turns.slice(-keep) };
}

/** v1 turns.js renderBody: a turn landing while you read never pushes what you read behind Show earlier (F-page-44).
 *  `from`: the first turn shown when the reader left the end; the window keeps it and grows instead of sliding. */
export function foldedFrom<T extends { readonly id: string }>(turns: readonly T[], open: boolean, from: string | undefined, keep = 3): { hidden: number; shown: readonly T[] } {
  const f = folded(turns, open, keep);
  if (from === undefined || f.hidden === 0) return f;
  const i = turns.findIndex((t) => t.id === from);
  return i < 0 || i >= f.hidden ? f : { hidden: i, shown: turns.slice(i) };
}

/** The crew member a task is a job of (`body.ext["wisp.crew"].id`), or undefined. */
export function crewOf(s: Pick<SessionRow, 'body'>): string | undefined {
  const c = (s.body?.['ext'] as Readonly<Record<string, unknown>> | undefined)?.['wisp.crew'] as { id?: unknown } | undefined;
  return typeof c?.id === 'string' ? c.id : undefined;
}

/** A row whose fields live in its `body` (a `crew_member`: `{id, name, updated_at, body}`), read flat; a flat row is its own. */
export function flatRow<T>(row: unknown): T {
  const r = (row ?? {}) as Record<string, unknown>;
  const body = r['body'];
  if (!body || typeof body !== 'object') return r as T;
  const { body: _b, ...top } = r;
  return { ...(body as Record<string, unknown>), ...top } as T;
}

/** Where Back goes from a task on a phone (v1 #773): a subagent's run goes to its lead (on the same computer), any
 * other task back through history (undefined). */
export function backOf(key: string, row: { readonly parent_id?: string | null } | undefined): string | undefined {
  const parent = row?.parent_id;
  if (!parent) return undefined;
  const at = key.indexOf(':');
  return at > 0 ? `${key.slice(0, at)}:${parent}` : parent;
}

/** A project as the screens show it: wisp.runtime's `project` row (`{id: name, body: spec}`) flattened. */
export interface ProjectView {
  readonly id: string;
  readonly name: string;
  readonly path: string;
  readonly color: string;
  /** the computer it's on (its entity key's peer), when the caller passed the keys */
  readonly peer?: string;
}

/** The setting that keeps your project order (v1 #772): one list of names, used everywhere projects show. */
export const PROJECT_ORDER = 'runtime.projectOrder';

/** The projects in your order: the ones you placed first, as you placed them, then the rest by name. */
export function projectList(rows: readonly unknown[], order: unknown, keys?: readonly string[]): ProjectView[] {
  const placed = Array.isArray(order) ? order.filter((x): x is string => typeof x === 'string') : [];
  const all = rows.map((r, i): ProjectView => {
    const p = flatRow<{ id?: unknown; name?: unknown; path?: unknown; color?: unknown }>(r);
    const id = String(p.id ?? '');
    // a folder as one string: `wisp add .` kept "/repo/." (and a trailing "/"), which then held none of its tasks
    const path = String(p.path ?? '')
      .replace(/(\/\.)+$/, '')
      .replace(/(.)\/+$/, '$1');
    // the entity key is `<peer>:<id>` (the store keys rows by origin): the computer the project is on
    const key = keys?.[i];
    const peer = key && key.includes(':') ? key.slice(0, key.indexOf(':')) : undefined;
    return { id, name: typeof p.name === 'string' && p.name ? p.name : id, path, color: String(p.color ?? ''), ...(peer ? { peer } : {}) };
  });
  const at = (id: string): number => {
    const i = placed.indexOf(id);
    return i < 0 ? Number.MAX_SAFE_INTEGER : i;
  };
  return all.filter((p) => p.id).sort((a, b) => at(a.id) - at(b.id) || a.name.localeCompare(b.name));
}

/** One projects list's folder lookups: each root normalized once (trailing slashes off, an empty one dropped), keyed
 *  by folder, and each folder's answer kept. ⌘K's task rows, the Projects counts and the need-you counts ask once per
 *  task on every rebuild; a regex, a filter and a sort over every project per task took ~141 ms at 5k tasks × 200
 *  projects (perf-r1 #2). */
interface ProjectIndex {
  /** a normalized root → its project: the deepest-first winner when several share it */
  readonly byRoot: Map<string, ProjectView>;
  readonly seen: Map<string, ProjectView | undefined>;
  /** an id or a name → the first project in the list that has it (a task's `project` fallback) */
  readonly byName: Map<string, ProjectView>;
}

// by the list's identity: a new list (a project added, renamed or moved) is a new index, never a stale answer
const indexes = new WeakMap<readonly ProjectView[], ProjectIndex>();

function projectIndex(projects: readonly ProjectView[]): ProjectIndex {
  let ix = indexes.get(projects);
  if (ix) return ix;
  const roots = projects
    .map((p, i) => ({ p, i, r: p.path.replace(/\/+$/, '') }))
    .filter((x) => x.r)
    // the deepest first; on one folder, the longer path as written (as the sort over every project did), then list order
    .sort((a, b) => b.r.length - a.r.length || b.p.path.length - a.p.path.length || a.i - b.i);
  const byRoot = new Map<string, ProjectView>();
  for (const x of roots) if (!byRoot.has(x.r)) byRoot.set(x.r, x.p);
  const byName = new Map<string, ProjectView>();
  for (const p of projects) for (const n of [p.id, p.name]) if (!byName.has(n)) byName.set(n, p);
  ix = { byRoot, seen: new Map(), byName };
  indexes.set(projects, ix);
  return ix;
}

/** The project a folder is in (v1 `projectFor`): the one whose path holds it, the deepest when they nest. */
export function projectOf(cwd: string | undefined, projects: readonly ProjectView[]): ProjectView | undefined {
  if (!cwd) return undefined;
  const ix = projectIndex(projects);
  if (ix.seen.has(cwd)) return ix.seen.get(cwd);
  // the folder itself, then each parent up (a root holds it when it's the folder or ends at one of its slashes):
  // the first found is the deepest
  let found = ix.byRoot.get(cwd);
  for (let i = cwd.lastIndexOf('/'); !found && i > 0; i = cwd.lastIndexOf('/', i - 1)) found = ix.byRoot.get(cwd.slice(0, i));
  ix.seen.set(cwd, found);
  return found;
}

/** The project a task belongs to: a worktree run's is the repo its worktree was made from (wisp.worktrees' `root`;
 *  its own folder is outside the project), else the one its folder is in, else the one its summary names (`project`:
 *  that repo's folder name). */
export function projectOfRow(s: { readonly summary?: unknown; readonly body?: unknown }, projects: readonly ProjectView[]): ProjectView | undefined {
  const sum = obj(s.summary) ?? {};
  const root = obj(obj(obj(s.body)?.['ext'])?.['wisp.worktrees'])?.['root'];
  const fromRoot = typeof root === 'string' && root ? projectOf(root, projects) : undefined;
  if (fromRoot) return fromRoot;
  const fromCwd = projectOf(typeof sum['cwd'] === 'string' ? sum['cwd'] : undefined, projects);
  if (fromCwd) return fromCwd;
  const named = sum['project'];
  return typeof named === 'string' && named ? projectIndex(projects).byName.get(named) : undefined;
}

/** How many tasks each project has (Manage projects' "N tasks"): each task counted for the project it belongs to
 *  (projectOfRow: a worktree task's folder is outside its project, audit C1). */
export function projectCounts(sessions: readonly { readonly summary?: unknown; readonly body?: unknown }[], projects: readonly ProjectView[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const s of sessions) {
    const p = projectOfRow(s, projects);
    if (p) out.set(p.id, (out.get(p.id) ?? 0) + 1);
  }
  return out;
}

/** The order after dragging `id` to where `to` is (before it; at the end when `to` is undefined). */
export function movedOrder(current: readonly ProjectView[], id: string, to: string | undefined): string[] {
  const ids = current.map((p) => p.id).filter((x) => x !== id);
  const i = to === undefined ? -1 : ids.indexOf(to);
  ids.splice(i < 0 ? ids.length : i, 0, id);
  return ids;
}

/** v1 #… (feed.js next): after Mark done, archive or delete, the task that needs you next (the newest in Needs you,
 *  not archived, not a subagent's, not the one just left), else none (New task). */
export function nextNeedsYou(rows: readonly SessionRow[], leaving: string): string | undefined {
  const bare = (id: string): string => id.slice(id.indexOf(':') + 1);
  const left = bare(leaving);
  return rows
    .filter((s) => bare(s.id) !== left && !s.archived && !s.parent_id && sectionOf(s) === 'needs')
    .reduce<SessionRow | undefined>((best, s) => (!best || s.updated_at > best.updated_at ? s : best), undefined)?.id;
}

/** A finished turn's activity counts for its trail bar (v1 renderTrail): only the kinds it did; steps of other kinds
 *  only: "Activity"; no steps (a turn that never ran, B9) or the transcript showing them already: none. */
export function trailActs(steps: number, n: { readonly read: number; readonly edit: number; readonly run: number }, t: ShellContext['t']): { icon?: 'read' | 'pen' | 'terminal'; text: string; title?: string }[] {
  if (!steps) return [];
  const acts = [
    { icon: 'read' as const, text: String(n.read), title: t('shell.task.read', { n: n.read }), n: n.read },
    { icon: 'pen' as const, text: String(n.edit), title: t('shell.task.edited', { n: n.edit }), n: n.edit },
    { icon: 'terminal' as const, text: String(n.run), title: t('shell.task.ran', { n: n.run }), n: n.run },
  ]
    .filter((a) => a.n > 0)
    .map(({ n: _n, ...a }) => a);
  return acts.length ? acts : [{ text: t('shell.task.activityLabel') }];
}

/** A session row's entity key (`<peer>:<id>`): `entities.list` gives the rows with their bare ids, `entities.keys`
 *  the keys; navigation and commands need the key (a bare id opened a blank task page from Home's Needs you). */
export function sessionKey(keys: readonly string[], id: string): string {
  if (id.includes(':')) return id;
  return keyOfId(keys, id) ?? id; // one index per keys list, shared: a scan per id made ⌘K O(n²) (perf-ui-r2 #1)
}
