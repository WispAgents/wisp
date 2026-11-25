// The harness picker's model (v1 runner-picker.js), DOM-free: the left pane's rows (Auto first, then each harness with
// its sub-line), which harness the right pane shows on open, what that pane lists (Auto's goals, or the harness's models
// grouped by provider and narrowed by the search), and what a pick changes. The kit's HarnessPicker draws it.
import { offerState, type AgentModel, type AgentOffer, type Runner } from './runner.ts';

export const AUTO = 'auto';
/** Auto's goals (v1 runners.js AUTO_GOALS), what its right pane lists in place of models */
export const AUTO_GOALS = ['balanced', 'cheap', 'best'] as const;
export type AutoGoal = (typeof AUTO_GOALS)[number];
export const goalOf = (model: string | undefined): AutoGoal => (AUTO_GOALS as readonly string[]).includes(model ?? '') ? (model as AutoGoal) : 'balanced';

/** A row's sub-line, as a string key and its values: the words are the caller's (t()). */
export type SubLine =
  | { readonly k: 'auto' }
  | { readonly k: 'checking' }
  | { readonly k: 'signedOut' }
  | { readonly k: 'missing' }
  /** how full its fullest window is, and in how long it resets (ms) when the harness says; `full`: at its limit */
  | { readonly k: 'used'; readonly label: string; readonly pct: number; readonly resetsIn?: number; readonly full?: true }
  | { readonly k: 'models'; readonly n: number }
  | { readonly k: 'default' }
  /** its plan was read and it has no window: "limits not reported by Cursor" (harness audit r2, ADR-0119) */
  | { readonly k: 'unreported' }
  /** nothing to say yet: its usage hasn't been read (UX 2026-11-18 #9: the model count was noise when picking a harness) */
  | { readonly k: 'none' };

export interface PickerRow {
  readonly id: string;
  readonly name: string;
  readonly sub: SubLine;
  /** dimmed (v1 .pk-h.off): not set up or signed out; still focusable, its pane says how to get it */
  readonly off: boolean;
  readonly auto: boolean;
}

/** v1 sub(): "Picks per task" for Auto, "4 models" (the Default not counted), "Default model" with none, why it
 * can't run, or how full its plan is (v1 usageNote) in place of the count. A harness still detecting says Checking…
 * (ADR-0066). */
export function subLine(a: AgentOffer, used?: UsedWindow | null): SubLine {
  const s = offerState(a);
  if (s === 'checking') return { k: 'checking' };
  if (s === 'signedOut') return { k: 'signedOut' };
  if (s === 'missing') return { k: 'missing' };
  // null: read, and the CLI gives no window; undefined: not read yet
  if (used === null) return { k: 'unreported' };
  if (!used) return { k: 'none' };
  return {
    k: 'used',
    label: used.label,
    pct: used.pct,
    ...(used.resetsIn !== undefined && used.resetsIn > 0 && { resetsIn: used.resetsIn }),
    ...(used.pct >= 100 && { full: true as const }),
  };
}

/** A harness's fullest window as the picker reads it: its label, whole percent used, and ms until it resets. */
export interface UsedWindow {
  readonly label: string;
  readonly pct: number;
  readonly resetsIn?: number | undefined;
}

/** The left pane: Auto first when it's offered, then each harness that can run, in the order given (UX 2026-11-18 #9:
 *  one that isn't installed is a choice that can't run; Set up another agent… under the list is the way to one). The
 *  one in use stays even when it's gone, so the pill's harness is there with why it can't run. */
export function pickerRows(
  offers: readonly AgentOffer[],
  o: { readonly auto: boolean; readonly autoName: string; readonly used?: (id: string) => UsedWindow | null | undefined; readonly inUse?: string },
): PickerRow[] {
  const rows = offers.filter((a) => offerState(a) !== 'missing' || a.id === o.inUse).map((a): PickerRow => {
    const s = offerState(a);
    const sub = subLine(a, s === 'ready' ? o.used?.(a.id) : undefined);
    // at its limit it's greyed like one that can't run, still listed with when it resets
    const full = sub.k === 'used' && sub.full === true;
    return { id: a.id, name: a.name, sub, off: full || s === 'missing' || s === 'signedOut' || (s === 'checking' && !a.available), auto: false };
  });
  return o.auto ? [{ id: AUTO, name: o.autoName, sub: { k: 'auto' }, off: false, auto: true }, ...rows] : rows;
}

/** The harness the right pane shows when the picker opens (v1 focusAgent): the one in use when it's there and usable
 * (Auto counts), else the first usable one, else the first row. */
export function firstFocus(rows: readonly PickerRow[], inUse: string): string {
  // the one in use, also while it's still checking (ADR-0066): the pane must not jump to it once its detect lands
  const own = rows.find((r) => r.id === inUse && (!r.off || r.sub.k === 'checking'));
  return (own ?? rows.find((r) => !r.off) ?? rows[0])?.id ?? '';
}

/** The next row from `at` by `d` (±1), going round (v1 step). */
export function stepRow(ids: readonly string[], at: string, d: 1 | -1): string | undefined {
  if (ids.length === 0) return undefined;
  const i = ids.indexOf(at);
  if (i < 0) return d > 0 ? ids[0] : ids[ids.length - 1];
  return ids[(i + d + ids.length) % ids.length];
}

export interface ModelGroup {
  /** opencode-style ids (provider/model) group under their provider; '' for the rest */
  readonly head: string;
  readonly items: readonly AgentModel[];
}

/** v1 paintModels + filter: the Default first, then the models; ids with a `/` grouped by their hint (the provider);
 * `query` narrows by id or words (a group with nothing left goes). */
export function modelGroups(a: AgentOffer | undefined, query: string): ModelGroup[] {
  const all: AgentModel[] = [{ id: '', label: '' }, ...(a?.models ?? []).filter((m) => m.id !== '')];
  const q = query.trim().toLowerCase();
  const groups = new Map<string, AgentModel[]>();
  for (const m of all) {
    if (q && !`${m.id} ${m.label} ${m.hint ?? ''}`.toLowerCase().includes(q)) continue;
    const g = m.id.includes('/') ? (m.hint ?? '') : '';
    groups.set(g, [...(groups.get(g) ?? []), m]);
  }
  return [...groups].map(([head, items]) => ({ head, items }));
}

/** v1: the search shows only past six models. */
export const searchShown = (a: AgentOffer | undefined): boolean => (a?.models.filter((m) => m.id !== '').length ?? 0) + 1 > 6;

/** A harness row pressed (v1 useAgent): it's used at once with its default model and level (the picker stays open);
 * the one in use, or one that can't run, changes nothing (its pane shows how to set it up or sign in). */
export function useHarness(value: Runner, id: string, offers: readonly AgentOffer[]): Runner | undefined {
  if (id === value.agent) return undefined;
  if (id === AUTO) return { agent: AUTO, model: '', effort: '', account: '' };
  const a = offers.find((x) => x.id === id);
  if (!a || offerState(a) === 'missing' || offerState(a) === 'signedOut' || (a.checking === true && !a.available)) return undefined;
  return { agent: id, model: '', effort: '', account: '' };
}

/** A model (or Auto's goal) picked on `focused`'s pane (v1 choose): the thinking level carries over when the new model
 * takes it, else Default; a pick on another harness than the one in use moves to it. */
export function withModel(value: Runner, focused: string, model: string, offers: readonly AgentOffer[]): Runner {
  if (focused === AUTO) return { agent: AUTO, model, effort: '', account: '' };
  const a = offers.find((x) => x.id === focused);
  const m = a?.models.find((x) => x.id === model);
  const levels = m?.efforts ?? a?.efforts ?? [];
  const keep = focused === value.agent && levels.includes(value.effort) ? value.effort : '';
  return { agent: focused, model, effort: keep, account: focused === value.agent ? (value.account ?? '') : '', ...(m && m.id !== '' && { modelLabel: m.label }) };
}

/** A thinking level or account picked on `focused`'s pane commits in place (v1 chooseEffort/chooseAccount), moving to
 * that harness if it wasn't the one in use. */
export function withSetting(value: Runner, focused: string, change: { readonly effort?: string; readonly account?: string; readonly accountLabel?: string }): Runner {
  const base: Runner = focused === value.agent ? value : { agent: focused, model: '', effort: '', account: '' };
  return { ...base, ...change };
}

/** What's different (Try it two ways, v1 runner-picker.js paintVariant): which setting a row changes. */
export type VaryKey = 'subagentModel' | 'skills';

export interface VaryRow {
  readonly id: VaryKey;
  /** '' (the task's model), 'haiku', 'sonnet'; or 'on', 'off' */
  readonly options: readonly string[];
  readonly value: string;
}

/** The rows the pane shows under the rest for a copy of the lead (`can`: its harness's `variants`): Subagents showing
 * what the run would really use (the setting included), then Skills (on unless turned off). None: no group. */
export function varyRows(can: readonly string[], v: { readonly subagentModel?: string; readonly skills?: string } | undefined, setting: string): VaryRow[] {
  const out: VaryRow[] = [];
  if (can.includes('subagentModel')) out.push({ id: 'subagentModel', options: ['', 'haiku', 'sonnet'], value: v?.subagentModel ?? setting });
  if (can.includes('skills')) out.push({ id: 'skills', options: ['on', 'off'], value: v?.skills === 'off' ? 'off' : 'on' });
  return out;
}

/** A segment pressed on a What's different row (v1 chooseVariant): the run's new variant; Skills on keeps no key. */
export function withVary(v: { readonly subagentModel?: string; readonly skills?: string } | undefined, row: VaryKey, option: string): { subagentModel?: string; skills?: string } {
  if (row === 'subagentModel') return { ...v, subagentModel: option };
  const { skills: _skills, ...rest } = v ?? {};
  return option === 'off' ? { ...rest, skills: 'off' } : rest;
}

/** A runner + model pick, as Recent and Favourites keep it ('' model: the harness's Default). */
export interface PickRef {
  readonly agent: string;
  readonly model: string;
}

/** What the picker remembers per peer (the `runtime.picker` setting): the last picks and the starred ones. */
export interface PickerMemory {
  readonly recent: readonly PickRef[];
  readonly favourites: readonly PickRef[];
}

export const PICKER_SETTING = 'runtime.picker';
/** Recent keeps this many picks, the latest first. */
export const RECENT_MAX = 5;

const isRef = (x: unknown): x is PickRef =>
  typeof x === 'object' && x !== null && typeof (x as PickRef).agent === 'string' && (x as PickRef).agent !== '' && typeof (x as PickRef).model === 'string';
const same = (a: PickRef, b: PickRef): boolean => a.agent === b.agent && a.model === b.model;

/** The setting's value read leniently: anything that isn't a list of picks is none. */
export function pickerMemory(v: unknown): PickerMemory {
  const o = typeof v === 'object' && v !== null ? (v as { recent?: unknown; favourites?: unknown }) : {};
  const list = (x: unknown): PickRef[] => (Array.isArray(x) ? x.filter(isRef).map((r) => ({ agent: r.agent, model: r.model })) : []);
  return { recent: list(o.recent).slice(0, RECENT_MAX), favourites: list(o.favourites) };
}

/** A pick made: it goes first in Recent, once (an earlier copy leaves), and Recent keeps the latest five. Auto and a
 *  factory aren't a runner + model, so they don't go in. */
export function withRecent(m: PickerMemory, pick: PickRef): PickerMemory {
  if (pick.agent === AUTO || !pick.agent) return m;
  return { ...m, recent: [pick, ...m.recent.filter((r) => !same(r, pick))].slice(0, RECENT_MAX) };
}

/** A star pressed: a favourite stops being one, anything else becomes one (last). */
export function toggleFavourite(m: PickerMemory, pick: PickRef): PickerMemory {
  const on = m.favourites.some((f) => same(f, pick));
  return { ...m, favourites: on ? m.favourites.filter((f) => !same(f, pick)) : [...m.favourites, pick] };
}

export const isFavourite = (m: PickerMemory, pick: PickRef): boolean => m.favourites.some((f) => same(f, pick));

/** An option id in the find results and Recent/Favourites: the harness and the model, which the pane hands back. */
export const optionId = (p: PickRef): string => `${p.agent}\t${p.model}`;
export const refOf = (id: string): PickRef => {
  const at = id.indexOf('\t');
  return at < 0 ? { agent: id, model: '' } : { agent: id.slice(0, at), model: id.slice(at + 1) };
};

/** How well `text` matches `q` (both lower case): whole 100, a prefix 90, a word's prefix 70, inside 50, its letters in
 *  order 20, else 0. */
export function matchScore(text: string, q: string): number {
  const t = text.toLowerCase();
  if (!q || !t) return 0;
  if (t === q) return 100;
  if (t.startsWith(q)) return 90;
  if (t.split(/[\s\-_./:·]+/).some((w) => w.startsWith(q))) return 70;
  if (t.includes(q)) return 50;
  let i = 0;
  for (const c of t) if (c === q[i] && ++i === q.length) return 20;
  return 0;
}

export interface FindItem extends PickRef {
  readonly label: string;
  readonly hint?: string | undefined;
  readonly score: number;
}

export interface FindGroup {
  readonly agent: string;
  readonly name: string;
  readonly items: readonly FindItem[];
}

/** The find box's results: every harness's models (its Default included) that match `query`, grouped by harness, the
 *  best match first in each and the group with the best match first. A model's own match outranks its harness's name
 *  matching (the name only lifts all its models to a sixth of its score, under any model match), so "son" finds Sonnet before "Codex"'s. A
 *  harness that can't run isn't searched. */
export function findAll(offers: readonly AgentOffer[], query: string, defaultLabel: string): FindGroup[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: FindGroup[] = [];
  for (const a of offers) {
    const st = offerState(a);
    if (st === 'missing' || st === 'signedOut') continue;
    const byName = Math.floor(matchScore(a.name, q) / 6);
    const all: AgentModel[] = [{ id: '', label: defaultLabel }, ...a.models.filter((m) => m.id !== '')];
    const items = all
      .map((m): FindItem => {
        const own = Math.max(matchScore(m.label, q), matchScore(m.id, q), m.hint ? Math.floor(matchScore(m.hint, q) / 2) : 0);
        return { agent: a.id, model: m.id, label: m.label || defaultLabel, hint: m.hint, score: Math.max(own, byName) };
      })
      .filter((x) => x.score > 0)
      .sort((x, y) => y.score - x.score);
    if (items.length > 0) out.push({ agent: a.id, name: a.name, items });
  }
  return out.sort((x, y) => (y.items[0]?.score ?? 0) - (x.items[0]?.score ?? 0));
}

/** The left pane's Recent & favourites row (first, when either has a pick): its pane lists both. */
export const QUICK = 'wisp:quick';
