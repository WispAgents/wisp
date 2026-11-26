// The Usage page's model (UsageView.tsx), DOM-free so tests read it: the tiles' label, the bars per local day or ISO
// week (one colour, or stacked by harness), cache hit, each harness's limit line, and how a cost reads: `~$x.xx` when
// it's a list-price estimate (ADR-0124), with `+` when only some of the turns had a price.

import { untilLabel } from '@wisp/ui/pure';
import type { AccountPlan, Usage, UsageDay, UsageRow, UsageWeek } from './commands.ts';
import { pct } from './drain.ts';
import { HARNESSES, harnessesOf, tokenWords, type HarnessBudgetMeta } from './plan.ts';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

const DAY = 24 * 3600_000;
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'] as const;
const money = (n: number): string => `$${n.toFixed(2)}`;
/** "Nov 12" of a `YYYY-MM-DD` */
export const dayLabel = (d: string): string => `${MONTHS[Number(d.slice(5, 7)) - 1] ?? ''} ${Number(d.slice(8, 10))}`;

/** A cost as it reads: `$1.20`, `~$1.20` when it holds a list-price estimate, and `+` when some turns had no price
 *  (it's at least this much). */
export function costWords(cost: number, o: { readonly estimated?: boolean | undefined; readonly priced?: number; readonly turns?: number } = {}): string {
  const partial = o.priced !== undefined && o.turns !== undefined && o.priced > 0 && o.priced < o.turns;
  return `${o.estimated ? '~' : ''}${money(cost)}${partial ? '+' : ''}`;
}

/** A row's cost cell and its tooltip: '—' with nothing priced; what's estimated and what's partial, said. */
export function costCell(r: Pick<UsageRow, 'cost' | 'priced' | 'turns' | 'costEstimated' | 'estimated'>): { text: string; tip: string } {
  if (!r.priced) return { text: '—', tip: '' };
  const tips: string[] = [];
  if (r.costEstimated) tips.push(t('budget.view.estimatedTip', { count: r.estimated ?? r.priced }));
  if (r.priced < r.turns) tips.push(t('budget.view.partialTip', { priced: r.priced, turns: r.turns }));
  return { text: costWords(r.cost, { estimated: r.costEstimated, priced: r.priced, turns: r.turns }), tip: tips.join(' ') };
}

export type ChartMetric = 'cost' | 'tokens' | 'turns';
export type UsageScale = 'day' | 'week';

/** The Monday of a `YYYY-MM-DD`'s ISO week, as the runtime's `weekly` keys it (its `start`). */
export function weekStart(date: string): string {
  const at = Date.UTC(Number(date.slice(0, 4)), Number(date.slice(5, 7)) - 1, Number(date.slice(8, 10)));
  const dow = (new Date(at).getUTCDay() + 6) % 7; // 0 = Monday
  return new Date(at - dow * DAY).toISOString().slice(0, 10);
}

type Point = { readonly key: string; readonly cost: number; readonly tokens: number; readonly turns: number; readonly estimated: boolean };
const dayPoint = (d: UsageDay): Point => ({ key: d.date, cost: d.cost, tokens: d.tokens ?? 0, turns: d.turns, estimated: d.cost_estimated === true });
const weekPoint = (w: UsageWeek): Point => ({ key: w.start, cost: w.cost, tokens: w.tokens, turns: w.turns, estimated: w.cost_estimated === true });

/** The bars' points: a local day each, or the runtime's ISO-week rollup (`weekly`) when the scale is week. */
function points(u: Usage, scale: UsageScale): Point[] {
  if (scale === 'week' && u.weekly?.length) return u.weekly.map(weekPoint);
  return (u.daily ?? []).map(dayPoint);
}

const scaleLabel = (k: string, scale: UsageScale): string => (scale === 'week' ? t('budget.view.weekOf', { day: dayLabel(k) }) : dayLabel(k));

/** The chart from one answer: pure, so a test reads it. */
export function usageModel(
  u: Usage,
  metric?: ChartMetric,
  scale: UsageScale = 'day',
): {
  byCost: boolean;
  byTokens: boolean;
  active: ChartMetric;
  bars: { key: string; share: number; tip: string }[];
  max: string;
  first: string;
  unpriced: number;
  tokens: number;
  harnesses: readonly UsageRow[];
  /** the first tile's label: Tokens when no turn has a price, never a Spend tile of token counts */
  spendLabel: string;
  /** the Spend tile's value: `~$x.xx` with an estimate in it, `+` when some turns had no price */
  spend: string;
} {
  const byCost = (u.priced ?? 0) > 0;
  const harnesses = harnessesOf(u);
  const tokens = u.tokens ?? harnesses.reduce((s, r) => s + (r.tokens ?? 0), 0);
  const pts = points(u, scale);
  const byTokens = tokens > 0 && pts.some((d) => d.tokens > 0);
  const active: ChartMetric =
    metric === 'cost' && byCost ? 'cost' : metric === 'tokens' && byTokens ? 'tokens' : metric === 'turns' ? 'turns' : byCost ? 'cost' : byTokens ? 'tokens' : 'turns';
  const val = (d: Point): number => (active === 'cost' ? d.cost : active === 'tokens' ? d.tokens : d.turns);
  const top = Math.max(0, ...pts.map(val)) || 1;
  // the tallest bar's cost is an estimate when the bar holds one
  const topEstimated = pts.some((d) => val(d) === top && d.estimated);
  const maxStr = active === 'cost' ? costWords(top, { estimated: topEstimated }) : active === 'tokens' ? tokenWords(top) : String(top);
  return {
    byCost,
    byTokens,
    active,
    bars: pts.map((d) => {
      const parts: string[] = [];
      if (byCost && d.cost > 0) parts.push(costWords(d.cost, { estimated: d.estimated }));
      if (d.tokens > 0) parts.push(t('budget.usage.harness.tokens', { n: tokenWords(d.tokens) }));
      parts.push(t('budget.view.turns', { count: d.turns }));
      return { key: d.key, share: val(d) / top, tip: `${scaleLabel(d.key, scale)}: ${parts.join(' · ')}` };
    }),
    max: scale === 'week' ? t('budget.view.maxWeek', { v: maxStr }) : t('budget.view.max', { v: maxStr }),
    first: pts[0] ? scaleLabel(pts[0].key, scale) : '',
    unpriced: u.turns - (u.priced ?? 0),
    tokens,
    harnesses,
    spendLabel: !byCost && tokens > 0 ? t('budget.usage.tokens') : t('budget.usage.spend'),
    spend: costWords(u.spend, { estimated: u.costEstimated, priced: u.priced ?? 0, turns: u.turns }),
  };
}

/** The first tile, on Settings → Usage and the Usage page alike: Spend as `costWords` says it (`~` an estimate, `+`
 *  some turns unpriced), Tokens when no turn has a price (never "Spend $0.00" of nothing priced), and '—' before the
 *  answer or with nothing at all. */
export function spendTile(u: Usage | undefined): { label: string; value: string } {
  if (!u) return { label: t('budget.usage.spend'), value: '—' };
  const m = usageModel(u);
  if (m.byCost) return { label: m.spendLabel, value: m.spend };
  return { label: m.spendLabel, value: m.tokens > 0 ? tokenWords(m.tokens) : u.turns > 0 ? t('budget.usage.spend.notPriced') : '—' };
}

export interface StackSeries {
  readonly key: string;
  readonly label: string;
  readonly series: number;
  readonly off: boolean;
}

type Cell = { cost: number; tokens: number; turns: number; estimated: boolean };
const EMPTY: Cell = { cost: 0, tokens: 0, turns: 0, estimated: false };

/** The stacked chart from one answer (`dailyByHarness`): a colour per harness in a fixed order (a harness keeps its
 * colour when another is hidden), a bar per day or per ISO week (the weeks the runtime's `weekly` names), each split
 * bottom up by harness. A hidden series leaves the bars and the scale. Pure, so a test reads it. */
export function stackedModel(
  u: Usage,
  metric: ChartMetric,
  opts: { readonly hidden?: ReadonlySet<string>; readonly scale?: UsageScale; readonly nameOf?: (agent: string) => string } = {},
): {
  series: readonly StackSeries[];
  bars: { key: string; share: number; tip: string; stack: { series: number; share: number }[] }[];
  max: string;
  first: string;
} {
  const hidden = opts.hidden ?? new Set<string>();
  const scale = opts.scale ?? 'day';
  const nameOf = opts.nameOf ?? ((a: string) => HARNESSES.find((h) => h.agent === a)?.name ?? a);
  const rows = u.dailyByHarness ?? [];
  const order = (a: string): number => {
    const i = HARNESSES.findIndex((h) => h.agent === a);
    return i < 0 ? HARNESSES.length : i;
  };
  const agents = [...new Set(rows.map((r) => r.agent))].sort((a, b) => order(a) - order(b) || a.localeCompare(b));
  const series = agents.map((a, i) => ({ key: a, label: nameOf(a), series: i, off: hidden.has(a) }));
  const val = (r: Cell): number => (metric === 'cost' ? r.cost : metric === 'tokens' ? r.tokens : r.turns);
  const keyOf = (date: string): string => (scale === 'week' ? weekStart(date) : date);
  // every bar of the window (empty ones too): its days, or the runtime's weeks
  const keys = [...new Set(points(u, scale).map((p) => keyOf(p.key)))];
  const cells = new Map<string, Map<string, Cell>>();
  for (const r of rows) {
    if (hidden.has(r.agent)) continue;
    const k = keyOf(r.date);
    const bar = cells.get(k) ?? new Map<string, Cell>();
    const cur = bar.get(r.agent) ?? EMPTY;
    bar.set(r.agent, {
      cost: cur.cost + r.cost,
      tokens: cur.tokens + r.tokens,
      turns: cur.turns + r.turns,
      estimated: cur.estimated || r.cost_estimated === true,
    });
    cells.set(k, bar);
  }
  const total = (k: string): number => [...(cells.get(k)?.values() ?? [])].reduce((s, x) => s + val(x), 0);
  const estimatedIn = (k: string): boolean => [...(cells.get(k)?.values() ?? [])].some((x) => x.estimated);
  const top = Math.max(0, ...keys.map(total)) || 1;
  // a cost holding a list-price estimate reads `~$x.xx` (costWords), in the tooltip and the scale alike
  const words = (v: number, estimated = false): string => (metric === 'cost' ? costWords(v, { estimated }) : metric === 'tokens' ? tokenWords(v) : String(v));
  const topEstimated = keys.some((k) => total(k) === top && estimatedIn(k));
  return {
    series,
    bars: keys.map((k) => {
      const bar = cells.get(k);
      const on = series
        .filter((x) => !x.off && bar?.has(x.key))
        .map((x) => {
          const c = bar?.get(x.key) ?? EMPTY;
          return { x, v: val(c), est: c.estimated };
        });
      return {
        key: k,
        share: total(k) / top,
        stack: on.map(({ x, v }) => ({ series: x.series, share: v / top })),
        tip: `${scaleLabel(k, scale)}: ${on.length ? on.map(({ x, v, est }) => `${x.label} ${words(v, est)}`).join(' · ') : words(0)}`,
      };
    }),
    max: scale === 'week' ? t('budget.view.maxWeek', { v: words(top, topEstimated) }) : t('budget.view.max', { v: words(top, topEstimated) }),
    first: keys[0] ? scaleLabel(keys[0], scale) : '',
  };
}

/** Cache hit: the share of the input read from prompt cache ('—' with no input, never NaN). */
export function cacheHit(r: { readonly inputTokens?: number | undefined; readonly cacheTokens?: number | undefined }): string {
  const inp = r.inputTokens ?? 0;
  return inp > 0 ? pct((r.cacheTokens ?? 0) / inp) : '—';
}

/** A harness card's limit line: the fullest window's share left and when it resets, or that its CLI reports none. */
export function limitLeft(h: HarnessBudgetMeta, accounts: readonly AccountPlan[], now: number): { left: string; resets: string; full: boolean } | { none: string } {
  const all = accounts.flatMap((a) => a.windows);
  const w = all.reduce<(typeof all)[number] | undefined>((best, x) => (!best || x.used > best.used ? x : best), undefined);
  if (!w) return { none: t('budget.usage.limits.none', { name: h.name }) };
  const left = t('budget.view.left', { label: w.label.charAt(0).toLowerCase() + w.label.slice(1), pct: pct(Math.max(0, 1 - w.used)) });
  const resets = w.resetsAt && w.resetsAt > now ? t('budget.view.resetsIn', { in: untilLabel(w.resetsAt - now) }) : '';
  return { left, resets, full: w.used >= 1 };
}
