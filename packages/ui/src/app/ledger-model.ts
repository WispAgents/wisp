// The task history as a ledger (ADR-0068, docs/design/history-redesign.md § A): every earlier turn one line led by its
// outcome, today and yesterday open, earlier days folded into a card, runs of wisp's notices folded into one line, a
// search and filters that turn it into a flat list grouped by day, and a scrubber of one tick per turn. Plain functions
// over plain data, so the shell, the kit and their tests share them (`@wisp/ui/pure`).

/** What came of a turn: its glyph and its colour. */
export type Outcome = 'done' | 'failed' | 'answer' | 'running';

export interface LedgerTurn {
  readonly id: string;
  /** its place in the task, from 1 */
  readonly n: number;
  /** when it started (ms) */
  readonly at: number;
  /** the answer's hero title; '' when it has none (the row shows the prompt's first line instead) */
  readonly hero: string;
  /** what was sent, whole */
  readonly prompt: string;
  readonly outcome: Outcome;
  /** set when wisp sent it (not the user): what kind of notice, in words ('CI finished', 'Scheduled follow-up') */
  readonly notice?: string;
  /** the paths it changed */
  readonly files: readonly string[];
  /** it committed, pushed or opened a PR */
  readonly shipped: boolean;
}

export type LedgerFilter = 'all' | 'yours' | 'failed' | 'shipped' | 'notices';
export const FILTERS: readonly LedgerFilter[] = ['all', 'yours', 'failed', 'shipped', 'notices'];

/** The scrubber shows only on a task with more than this many turns (the owner's pick, 2026-11-15). */
export const SCRUB_ABOVE = 30;
/** Past this many rows the ledger is a VirtualList (AGENTS.md: lists over 200 rows). */
export const VIRTUAL_ABOVE = 200;
/** A run of this many notices or more folds into one line. */
export const FOLD_RUN = 3;

const DAY = 86_400_000;

/** Local midnight of the day `at` falls on. */
export function dayOf(at: number): number {
  const d = new Date(at);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export type DayKind = 'today' | 'yesterday' | 'earlier';

export function dayKind(day: number, now: number): DayKind {
  const today = dayOf(now);
  if (day >= today) return 'today';
  // a day before today's midnight by less than a day and a bit (a DST change makes one 23 or 25 hours)
  return day >= dayOf(today - DAY / 2) ? 'yesterday' : 'earlier';
}

/** The first line of a text, trimmed. */
export const firstLine = (s: string): string => (s.trim().split('\n')[0] ?? '').trim();

/** The row's title: the hero, or the prompt's first line when there's none. */
export const titleOf = (t: LedgerTurn): string => t.hero || firstLine(t.prompt);

export function matches(t: LedgerTurn, q: string, filter: LedgerFilter): boolean {
  if (filter === 'yours' && t.notice !== undefined) return false;
  if (filter === 'notices' && t.notice === undefined) return false;
  if (filter === 'failed' && t.outcome !== 'failed') return false;
  if (filter === 'shipped' && !t.shipped) return false;
  const words = q.trim().toLowerCase();
  if (!words) return true;
  const hay = `${t.hero}\n${t.prompt}\n${t.notice ?? ''}\n${t.files.join('\n')}`.toLowerCase();
  // every word, anywhere: "reconnect test" finds "Reconnect timer … test"
  return words.split(/\s+/).every((w) => hay.includes(w));
}

export function counts(turns: readonly LedgerTurn[]): Readonly<Record<LedgerFilter, number>> {
  const c = { all: turns.length, yours: 0, failed: 0, shipped: 0, notices: 0 };
  for (const t of turns) {
    if (t.notice === undefined) c.yours++;
    else c.notices++;
    if (t.outcome === 'failed') c.failed++;
    if (t.shipped) c.shipped++;
  }
  return c;
}

/** One day's turns, in order. */
export interface Day {
  readonly day: number;
  readonly kind: DayKind;
  readonly turns: readonly LedgerTurn[];
}

/** Turns (oldest first) by local day, oldest day first. */
export function daysOf(turns: readonly LedgerTurn[], now: number): Day[] {
  // by day, not by run: a turn whose time is out of order (an import, a skewed clock) joins its own day, which stays one
  const by = new Map<number, { day: number; kind: DayKind; turns: LedgerTurn[] }>();
  for (const t of turns) {
    const day = dayOf(t.at);
    const d = by.get(day);
    if (d) d.turns.push(t);
    else by.set(day, { day, kind: dayKind(day, now), turns: [t] });
  }
  return [...by.values()].sort((a, b) => a.day - b.day);
}

export interface DayStats {
  readonly count: number;
  readonly from: number;
  readonly to: number;
  readonly done: number;
  readonly failed: number;
  /** distinct paths changed that day */
  readonly files: number;
}

export function dayStats(turns: readonly LedgerTurn[]): DayStats {
  const files = new Set<string>();
  let done = 0;
  let failed = 0;
  for (const t of turns) {
    if (t.outcome === 'done') done++;
    if (t.outcome === 'failed') failed++;
    for (const f of t.files) files.add(f);
  }
  return { count: turns.length, from: turns[0]?.at ?? 0, to: turns.at(-1)?.at ?? 0, done, failed, files: files.size };
}

/** A tick's look: on the day card's strip and the scrubber. */
export type Tone = 'done' | 'failed' | 'answer' | 'notice' | 'running' | 'on';

export function toneOf(t: LedgerTurn, open?: string): Tone {
  if (t.id === open) return 'on';
  if (t.outcome === 'failed') return 'failed';
  if (t.notice !== undefined) return 'notice';
  return t.outcome;
}

/** A line of the ledger, as the list draws it. */
export type LedgerRow =
  | { readonly k: 'day'; readonly key: string; readonly day: number; readonly kind: DayKind; readonly count: number; readonly search: boolean }
  | { readonly k: 'card'; readonly key: string; readonly day: number; readonly kind: DayKind; readonly stats: DayStats; readonly strip: readonly Tone[] }
  | { readonly k: 'turn'; readonly key: string; readonly turn: LedgerTurn }
  | { readonly k: 'fold'; readonly key: string; readonly turns: readonly LedgerTurn[]; readonly kinds: readonly string[] };

/** A day's turns with each run of [`FOLD_RUN`]+ notices as one fold line, unless it's unfolded (by its first turn's
 * id) or holds the open turn. */
export function foldNotices(turns: readonly LedgerTurn[], open: string | undefined, unfolded: ReadonlySet<string>): LedgerRow[] {
  const out: LedgerRow[] = [];
  let run: LedgerTurn[] = [];
  const flush = (): void => {
    const first = run[0];
    if (first && run.length >= FOLD_RUN && !unfolded.has(first.id) && !run.some((t) => t.id === open)) {
      out.push({ k: 'fold', key: `fold:${first.id}`, turns: run, kinds: [...new Set(run.map((t) => t.notice ?? ''))].filter(Boolean) });
    } else for (const t of run) out.push({ k: 'turn', key: `turn:${t.id}`, turn: t });
    run = [];
  };
  for (const t of turns) {
    if (t.notice !== undefined) run.push(t);
    else {
      flush();
      out.push({ k: 'turn', key: `turn:${t.id}`, turn: t });
    }
  }
  flush();
  return out;
}

export interface LedgerView {
  readonly q: string;
  readonly filter: LedgerFilter;
  /** days the reader opened or folded by hand (local midnight → open) */
  readonly days: ReadonlyMap<number, boolean>;
  readonly unfolded: ReadonlySet<string>;
  readonly open: string | undefined;
}

export const searching = (v: Pick<LedgerView, 'q' | 'filter'>): boolean => v.q.trim() !== '' || v.filter !== 'all';

/** Whether a day shows its turns: by hand if the reader chose, else today and yesterday, and always the open turn's. */
export function dayOpen(d: Day, v: Pick<LedgerView, 'days' | 'open'>): boolean {
  if (v.open !== undefined && d.turns.some((t) => t.id === v.open)) return true;
  return v.days.get(d.day) ?? d.kind !== 'earlier';
}

/** The ledger's lines: open days with their turns and notice folds, earlier days as cards; while searching or
 * filtering, the matches only, flat, under a heading per day. */
export function ledgerRows(turns: readonly LedgerTurn[], now: number, v: LedgerView): LedgerRow[] {
  const out: LedgerRow[] = [];
  if (searching(v)) {
    for (const d of daysOf(
      turns.filter((t) => matches(t, v.q, v.filter)),
      now,
    )) {
      out.push({ k: 'day', key: `day:${d.day}`, day: d.day, kind: d.kind, count: d.turns.length, search: true });
      for (const t of d.turns) out.push({ k: 'turn', key: `turn:${t.id}`, turn: t });
    }
    return out;
  }
  for (const d of daysOf(turns, now)) {
    if (!dayOpen(d, v)) {
      out.push({ k: 'card', key: `card:${d.day}`, day: d.day, kind: d.kind, stats: dayStats(d.turns), strip: d.turns.map((t) => toneOf(t)) });
      continue;
    }
    out.push({ k: 'day', key: `day:${d.day}`, day: d.day, kind: d.kind, count: d.turns.length, search: false });
    out.push(...foldNotices(d.turns, v.open, v.unfolded));
  }
  return out;
}

export interface Tick {
  readonly id: string;
  readonly tone: Tone;
  /** where it sits, 0 (the first turn) to 1 */
  readonly at: number;
}

/** The scrubber: one tick per turn, spread evenly, oldest at the top, each in the middle of the slot `turnAt` gives it. */
export function ticks(turns: readonly LedgerTurn[], open: string | undefined): Tick[] {
  const n = turns.length;
  return turns.map((t, i) => ({ id: t.id, tone: toneOf(t, open), at: (i + 0.5) / n }));
}

/** The turn under a point `y` px down a scrubber `h` px tall, or undefined when there are none. */
export function turnAt<T>(turns: readonly T[], y: number, h: number): T | undefined {
  if (turns.length === 0 || !(h > 0)) return undefined;
  const i = Math.floor((Math.min(Math.max(y, 0), h - 0.001) / h) * turns.length);
  return turns[Math.min(Math.max(i, 0), turns.length - 1)];
}

/** j/k: the turn after (+1) or before (-1) the open one among `turns`; with none open, the latest (k) or the first (j).
 * Undefined past either end. */
export function stepFrom(turns: readonly LedgerTurn[], open: string | undefined, dir: 1 | -1): LedgerTurn | undefined {
  if (turns.length === 0) return undefined;
  const i = open === undefined ? -1 : turns.findIndex((t) => t.id === open);
  if (i < 0) return dir < 0 ? turns.at(-1) : turns[0];
  return turns[i + dir];
}

/** [ / ]: the failed turn before (-1) or after (+1) the open one; with none open, the latest (-1) or the first (+1)
 * failure. */
export function nextFailure(turns: readonly LedgerTurn[], open: string | undefined, dir: 1 | -1): LedgerTurn | undefined {
  const at = open === undefined ? -1 : turns.findIndex((t) => t.id === open);
  const from = at < 0 ? (dir > 0 ? -1 : turns.length) : at;
  for (let i = from + dir; i >= 0 && i < turns.length; i += dir) {
    const t = turns[i];
    if (t?.outcome === 'failed') return t;
  }
  return undefined;
}

/** What the ledger's keys do; undefined: not a ledger key (it goes on to the page's shortcuts). */
export type LedgerKey = 'next' | 'prev' | 'search' | 'nextFail' | 'prevFail' | 'close';

export function ledgerKey(e: { readonly key: string; readonly metaKey?: boolean; readonly ctrlKey?: boolean; readonly altKey?: boolean }): LedgerKey | undefined {
  if (e.metaKey || e.ctrlKey || e.altKey) return undefined;
  switch (e.key) {
    case 'j':
      return 'next';
    case 'k':
      return 'prev';
    case '/':
      return 'search';
    case ']':
      return 'nextFail';
    case '[':
      return 'prevFail';
    case 'Escape':
      return 'close';
    default:
      return undefined;
  }
}
