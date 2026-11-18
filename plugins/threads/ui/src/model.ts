// The Thread tab's pure half (ADR-0079; v1 web/js/threads.js SECTIONS, features/thread.js): the seven sections in
// order with who wrote each, the text a rewrite keeps, the one health line, a section's revisions, and which tasks
// have a thread. No DOM, no signals: test/model.test.ts.
import type { Audit, AuditRow, By, Health, Level, MetaRow, NoteRow, Revision, RevisionRow } from './types.ts';
import './i18n/lazy.ts';

/** "2m ago" moves on while the Thread shows: its clock ticks this often */
export const TICK_MS = 30_000;

/** v1's order; `one`: one text (goal, state), else a list, one item a line (wisp-threads-core notes::META). */
export const SECTIONS = [
  { id: 'goal', one: true },
  { id: 'constraints', one: false },
  { id: 'decisions', one: false },
  { id: 'failed', one: false },
  { id: 'state', one: true },
  { id: 'open', one: false },
  { id: 'files', one: false },
] as const;
export type SectionId = (typeof SECTIONS)[number]['id'];

const IDS = new Set<string>(SECTIONS.map((s) => s.id));
export const isSection = (id: string): id is SectionId => IDS.has(id);
export const isOne = (id: string): boolean => SECTIONS.some((s) => s.id === id && s.one);

/** The server's limits (notes.rs ITEM_MAX, ONE_MAX, LIST_MAX). */
export const ITEM_MAX = 400;
export const ONE_MAX = 1200;
export const LIST_MAX = 20;

/** One section as the tab shows it. `rev` 0: never written. */
export interface SectionView {
  readonly id: SectionId;
  readonly one: boolean;
  readonly text: string;
  readonly rev: number;
  readonly by: By;
  readonly at: number;
}

/** A section's row id: `<task>|<section>` (store.rs). */
export const noteId = (task: string, section: string): string => `${task}|${section}`;

/** The task's sections in order, from its `threads_notes` rows (or `threads.notes`' sections); missing ones empty. */
type SectionLike = Pick<NoteRow, 'section' | 'text' | 'rev' | 'by' | 'at'> & { readonly task_id?: string };
export function sectionsOf(rows: readonly SectionLike[], task?: string): SectionView[] {
  const mine = rows.filter((r) => task === undefined || r.task_id === undefined || r.task_id === task);
  return SECTIONS.map((s) => {
    const r = mine.find((x) => x.section === s.id);
    return { id: s.id, one: s.one, text: r?.text ?? '', rev: r?.rev ?? 0, by: r?.by === 'you' ? 'you' : 'agent', at: r?.at ?? 0 };
  });
}

/** Variant A: the filled sections show; the empty ones fold into one "+ Add" row. */
export function splitSections(all: readonly SectionView[]): { filled: SectionView[]; empty: SectionView[] } {
  return { filled: all.filter((s) => s.text.trim() !== ''), empty: all.filter((s) => s.text.trim() === '') };
}

export const itemsOf = (text: string): string[] =>
  text
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean);

/** What the section shows, as markdown: a one-text section as written, a list as bullets. */
export function shownText(id: string, text: string): string {
  return isOne(id) ? text : itemsOf(text).map((x) => `- ${x}`).join('\n');
}

const cut = (s: string, max: number): string => {
  const chars = [...s];
  return chars.length <= max ? s : `${chars.slice(0, max - 1).join('')}…`;
};
/** "No Redis." and "no redis" are the same item. */
const itemKey = (s: string): string =>
  s
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
    .join(' ');
/** A list item: one line; a leading `- `, `* `, `• ` or `1. ` / `1) ` is the list's, not the item's. */
function item(line: string): string {
  let t = line.trim();
  for (const mark of ['- ', '* ', '• ']) if (t.startsWith(mark)) t = t.slice(mark.length);
  const n = /^(\d{1,3})[.)] /.exec(t);
  if (n) t = t.slice(n[0].length);
  return cut(t.trim(), ITEM_MAX);
}

/** The text a section keeps for `text` as written (wisp-threads-core notes::clean), so the optimistic text is the text. */
export function cleanText(id: string, text: string): string {
  const raw = text.replace(/\r\n?/g, '\n');
  if (isOne(id)) {
    return cut(
      raw
        .split('\n')
        .map((l) => l.trimEnd())
        .join('\n')
        .trim(),
      ONE_MAX,
    );
  }
  const items: string[] = [];
  for (const x of raw.split('\n').map(item)) if (x && !items.some((y) => itemKey(y) === itemKey(x))) items.push(x);
  return items.slice(-LIST_MAX).join('\n');
}

/** A health fact, for t(): `threads.fact.<what>` with its numbers. */
export interface Fact {
  readonly key: `threads.fact.${'context' | 'compactions' | 'repeat' | 'turns' | 'reverted' | 'stale'}`;
  readonly vars: Readonly<Record<string, number>>;
}

/** The signals' thresholds (ADR-0079 §6: what counts toward Getting long). */
export const HEALTH = { share: 0.6, compactions: 2, turns: 30, repeats: 3, reverted: 2, stale: 8 } as const;

/** What made the thread Getting long or Worn, worst first (none when Fine): the line after the level's word. */
export function healthFacts(h: Health): Fact[] {
  if (h.level === 'ok') return [];
  const out: Fact[] = [];
  if (h.share !== undefined && h.share >= HEALTH.share) out.push({ key: 'threads.fact.context', vars: { pct: Math.round(h.share * 100) } });
  if (h.compactions >= HEALTH.compactions) out.push({ key: 'threads.fact.compactions', vars: { count: h.compactions } });
  if (h.repeat && h.repeat.n >= HEALTH.repeats) out.push({ key: 'threads.fact.repeat', vars: { count: h.repeat.n } });
  if (h.turns >= HEALTH.turns) out.push({ key: 'threads.fact.turns', vars: { count: h.turns } });
  if (h.reverted >= HEALTH.reverted) out.push({ key: 'threads.fact.reverted', vars: { count: h.reverted } });
  if (h.notesAge !== undefined && h.notesAge >= HEALTH.stale && h.turns >= HEALTH.stale) out.push({ key: 'threads.fact.stale', vars: { count: h.notesAge } });
  return out;
}

/** The level's colour: state only (design/ui.md). */
export function toneOf(level: Level): 'good' | 'warn' | 'bad' {
  return level === 'worn' ? 'bad' : level === 'heavy' ? 'warn' : 'good';
}

/** A section's earlier versions, newest first, the one it holds now (`current`) left out. */
export function revisionsOf(rows: readonly RevisionRow[], task: string, section: string, current: number): Revision[] {
  return rows
    .filter((r) => r.task_id === task && r.section === section && r.rev !== current)
    .sort((a, b) => b.rev - a.rev)
    .map((r) => ({ section: r.section, rev: r.rev, text: r.text, by: r.by, at: r.at }));
}

/** The session row's fields this reads: as runtime syncs it (top level, the list's under `summary`), or flattened
 *  as the task's menu hands it to an extension (camelCase, the summary's fields on top). */
export interface TaskLike {
  readonly agent?: string;
  readonly state?: string;
  readonly parent_id?: string | null;
  readonly parentId?: string | null;
  readonly pending?: boolean;
  readonly kind?: string;
  readonly sideOf?: unknown;
  readonly summary?: { readonly pending?: boolean; readonly kind?: string; readonly sideOf?: unknown };
}

const field = <K extends 'pending' | 'kind' | 'sideOf'>(t: TaskLike, k: K): TaskLike[K] => t.summary?.[k] ?? t[k];

/** A task worth a Thread tab: one with an agent of its own (v1 threadable: not the tour, a spell, a pending one). */
export function threadable(t: TaskLike | undefined): boolean {
  return !!t && !!t.agent && t.agent !== 'wisp' && !field(t, 'pending') && !field(t, 'kind');
}

/** Fresh thread is for a task's own conversation once its turn has finished (v1 freshable). */
export function freshable(t: TaskLike | undefined): boolean {
  return !!t && threadable(t) && !(t.parent_id ?? t.parentId) && !field(t, 'sideOf') && t.state !== 'running' && t.state !== 'waiting';
}

/** You changed the Notes since the agent last read them (notes.rs yours_unseen: per section, v1's bug fixed). */
export function yoursUnseen(meta: MetaRow | undefined, sections: readonly SectionView[]): boolean {
  return !!meta && meta.rev > meta.seen && sections.some((s) => s.by === 'you' && s.rev > 0);
}

/** An audit still asking this long after it started was cut off (a restart): it reads as skipped (audit.rs STALE_MS). */
export const AUDIT_STALE_MS = 10 * 60_000;
/** The server's words for one cut off (audit.rs list). */
export const CUT_OFF = 'It didn’t finish.';

function settled(a: Audit, now: number): Audit {
  return a.status === 'running' && now - a.at > AUDIT_STALE_MS ? { ...a, status: 'skipped', why: CUT_OFF } : a;
}

/** A task's compaction audits, newest first, as `threads.audits` answers them. */
export function auditsOf(rows: readonly AuditRow[], task: string, now: number): Audit[] {
  return rows
    .filter((r) => r.task_id === task && r.body)
    .map((r) => settled(r.body, now))
    .sort((a, b) => b.at - a.at);
}

/** The audit of one compaction's turn. */
export function auditFor(rows: readonly AuditRow[], turn: string, now = 0): Audit | undefined {
  const r = rows.find((x) => x.body?.turn === turn);
  return r ? settled(r.body, now) : undefined;
}

/** A lost item as the Notes say it ("Constraints: No Redis"): its section's label and the item. */
export function lostItem(s: string): { section: string; text: string } {
  const i = s.indexOf(': ');
  return i > 0 && i < 40 ? { section: s.slice(0, i), text: s.slice(i + 2) } : { section: '', text: s };
}
