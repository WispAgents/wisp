// Settings → Agents as a list and the one picked (owner, settings audit r1): which harness is picked (kept in the
// Settings params as `harness:<id>`, so a reload, Back and the search land on it), each one's state dot, a harness
// whose plugin is off (still listed, Disabled, switched back on from its Advanced), its fullest usage window for the
// list's ring, its Defaults in `runtime.launch` (read and written through these, never as raw JSON) and its own Ask
// near the limit in `budget.limitHoldBy`. DOM-free, so tests read them.
import type { DetectRow, HarnessRow } from './types.ts';

/** The list's first entry: the rows for every harness at once (Update all, Re-check, git and GitHub). */
export const ALL = 'all';
const PREFIX = 'harness:';

/** The Settings filter that opens Agents on `id` (Settings' search sends the same). */
export const harnessFilter = (id: string): string => `${PREFIX}${id}`;

/** The pick a filter names: one of `ids`, else All harnesses. */
export function pickOf(filter: string | undefined, ids: readonly string[]): string {
  const id = filter?.startsWith(PREFIX) ? filter.slice(PREFIX.length) : undefined;
  return id !== undefined && ids.includes(id) ? id : ALL;
}

// ---------- the list's dot ----------

export type Status = 'ready' | 'signedOut' | 'missing' | 'checking' | 'disabled';
type Tone = 'neutral' | 'good' | 'warn' | 'bad';

/** The dot's state: Disabled first (its plugin is off: nothing else about it is live), then as the row says. `cli`: its
 *  CLI's own login as last asked (harnessState's). */
export function statusOf(h: Pick<HarnessRow, 'available' | 'signedIn' | 'checking' | 'disabled'>, cli?: boolean): Status {
  if (h.disabled) return 'disabled';
  if (h.checking) return 'checking';
  if (h.available) return cli === false ? 'signedOut' : 'ready';
  return h.signedIn === false ? 'signedOut' : 'missing';
}

export const STATUS_TONE: Readonly<Record<Status, Tone>> = { ready: 'good', signedOut: 'warn', missing: 'bad', checking: 'neutral', disabled: 'neutral' };

// ---------- a harness whose plugin is off ----------

/** A plugin as `plugins.list` has it. */
export interface PluginState {
  readonly id: string;
  readonly enabled: boolean;
}

/** A harness the plugins registered (the shell's HarnessUiDef: it stays registered while its plugin is off). */
export interface KnownHarness {
  readonly id: string;
  readonly name: string;
  readonly order?: number | undefined;
}

/** The plugin a harness comes from: `wisp.<id>` for a built-in, else one whose id ends in `.<id>`. */
export const pluginOf = (id: string, plugins: readonly PluginState[]): PluginState | undefined =>
  plugins.find((p) => p.id === `wisp.${id}`) ?? plugins.find((p) => p.id.endsWith(`.${id}`));

/** The rows with each harness whose plugin is off marked Disabled, and added when no row has it (runtime lists only
 *  the harnesses that are on): it stays in the list, so it can be switched back on from its Advanced. */
export function withDisabled(rows: readonly HarnessRow[], known: readonly KnownHarness[], plugins: readonly PluginState[]): HarnessRow[] {
  const off = (id: string): boolean => pluginOf(id, plugins)?.enabled === false;
  const out = rows.map((h) => (off(h.id) ? { ...h, disabled: true } : h));
  const have = new Set(out.map((h) => h.id));
  for (const k of known) {
    if (have.has(k.id) || !off(k.id)) continue;
    out.push({ id: k.id, name: k.name, ord: k.order ?? out.length, available: false, models: [], signIn: 'none', disabled: true });
  }
  return out;
}

// ---------- the ring: its fullest usage window ----------

export interface PlanWindow {
  readonly id: string;
  readonly label: string;
  readonly used: number;
}

/** The fullest window of a harness's accounts' plans (budget.view), or undefined while none reported one. */
export function fullest(plans: readonly { readonly windows: readonly PlanWindow[] }[]): PlanWindow | undefined {
  let best: PlanWindow | undefined;
  for (const p of plans) for (const w of p.windows) if (Number.isFinite(w.used) && (!best || w.used > best.used)) best = w;
  return best;
}

/** The ring's tone: red when used up, amber past the line, else the accent. */
export const ringTone = (used: number, line: number | null): 'bad' | 'warn' | 'accent' => (used >= 1 ? 'bad' : used >= (line ?? 0.9) ? 'warn' : 'accent');

// ---------- Defaults: runtime.launch ----------

/** `runtime.launch` (wisp.runtime's, New task's last pick): each harness's model, its name for the pill, its level. */
export interface Launch {
  readonly agent?: string;
  readonly models?: Readonly<Record<string, string>>;
  readonly labels?: Readonly<Record<string, string>>;
  readonly efforts?: Readonly<Record<string, string>>;
}

const strings = (v: unknown): Record<string, string> | undefined => {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return undefined;
  return Object.fromEntries(Object.entries(v as Record<string, unknown>).filter((e): e is [string, string] => typeof e[1] === 'string'));
};

/** The setting's value as a Launch (anything malformed dropped; any other field kept as it was). */
export function readLaunch(v: unknown): Launch & Record<string, unknown> {
  if (!v || typeof v !== 'object' || Array.isArray(v)) return {};
  const o = v as Record<string, unknown>;
  const models = strings(o['models']);
  const labels = strings(o['labels']);
  const efforts = strings(o['efforts']);
  return { ...o, ...(models && { models }), ...(labels && { labels }), ...(efforts && { efforts }) };
}

/** A harness's saved model and level ('' when none). */
export const defaultsOf = (v: unknown, id: string): { model: string; effort: string } => {
  const l = readLaunch(v);
  return { model: l.models?.[id] ?? '', effort: l.efforts?.[id] ?? '' };
};

const put = (m: Readonly<Record<string, string>> | undefined, id: string, value: string): Record<string, string> => {
  const { [id]: _was, ...rest } = m ?? {};
  return value === '' ? rest : { ...rest, [id]: value };
};

/** A harness's model set (its name for the pill with it), '' for the harness's own default: only this harness's entry
 *  changes; the other harnesses', the levels and New task's last harness stay as they were. */
export function withModel(v: unknown, id: string, model: string, label: string): Launch & Record<string, unknown> {
  const l = readLaunch(v);
  return { ...l, models: put(l.models, id, model), labels: put(l.labels, id, model === '' ? '' : label) };
}

/** A harness's level set ('' for its default): only this harness's entry changes. */
export function withEffort(v: unknown, id: string, effort: string): Launch & Record<string, unknown> {
  const l = readLaunch(v);
  return { ...l, efforts: put(l.efforts, id, effort) };
}

/** A saved model the harness no longer lists (renamed, retired): the Defaults row warns and offers Reset. Unknown while
 *  the harness lists nothing yet (still checking, not found). */
export const staleModel = (saved: string, models: readonly { readonly id: string }[]): boolean =>
  saved !== '' && models.length > 0 && !models.some((m) => m.id === saved);

/** The levels a harness offers for a model: the model's own when it says, else the harness's. */
export function effortsFor(h: Pick<HarnessRow, 'models' | 'efforts'>, model: string): readonly string[] {
  return h.models.find((m) => m.id === model)?.efforts ?? h.efforts ?? [];
}

/** The detect row's levels onto the harness (runtime's, ADR-0066): what Defaults offers. */
export const effortsOfDetect = (d: DetectRow | undefined): { efforts?: string[] } => (d?.agent.efforts ? { efforts: [...d.agent.efforts] } : {});

// ---------- Limits & budget: budget.limitHoldBy ----------

export const HOLDS = ['same', 'off', '80', '90', '95'] as const;
export type Hold = (typeof HOLDS)[number];

/** A harness's own Ask near the limit ('same': everyone's, `budget.limitHold`). */
export function holdOf(v: unknown, id: string): Hold {
  const own = strings(v)?.[id];
  return own !== undefined && (HOLDS as readonly string[]).includes(own) && own !== 'same' ? (own as Hold) : 'same';
}

/** A harness's own line set ('same' takes it out): the other harnesses' stay. */
export function withHold(v: unknown, id: string, hold: Hold): Record<string, string> {
  return put(strings(v), id, hold === 'same' ? '' : hold);
}
