// Settings → Agents as a list and a detail (harness audit r2): General first (GitHub and git, Lean runs, Ask before a
// changed harness, updates, Re-check), then one item per harness with its state dot. The search above the list keeps
// an item when its name or one of the settings it holds has every word typed. Pure, so tests read it.

import type { Tone } from './words.ts';

export const GENERAL = 'general';

export interface NavHarness {
  readonly id: string;
  readonly name: string;
  readonly state: { readonly tone: Tone; readonly label: string };
  /** its plan's fullest window is used up */
  readonly atLimit?: boolean;
  readonly logo?: string | undefined;
}

export interface NavItem {
  readonly id: string;
  readonly label: string;
  readonly sub?: string;
  readonly dot?: 'good' | 'warn' | 'bad' | 'muted';
  readonly logo?: string | undefined;
}

const DOT: Readonly<Record<Tone, NonNullable<NavItem['dot']>>> = { good: 'good', warn: 'warn', bad: 'bad', neutral: 'muted' };

/** What a harness's detail holds and what General holds, as titles (the search reads them). */
export interface NavRows {
  readonly general: { readonly label: string; readonly rows: readonly string[] };
  readonly harness: readonly string[];
  /** the sub-line of a harness at its limit */
  readonly atLimit: string;
}

const matches = (text: string, words: readonly string[]): boolean => {
  const t = text.toLowerCase();
  return words.every((w) => t.includes(w));
};

/** The list: General, then each harness (ready, signed out, missing or at its limit), narrowed by `query`. */
export function navItems(harnesses: readonly NavHarness[], query: string, rows: NavRows): NavItem[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const keep = (label: string, titles: readonly string[]): boolean => words.length === 0 || matches(label, words) || titles.some((r) => matches(`${label} ${r}`, words) || matches(r, words));
  const out: NavItem[] = [];
  if (keep(rows.general.label, rows.general.rows)) out.push({ id: GENERAL, label: rows.general.label });
  for (const h of harnesses) {
    if (!keep(h.name, rows.harness)) continue;
    out.push({
      id: h.id,
      label: h.name,
      sub: h.atLimit ? rows.atLimit : h.state.label,
      dot: h.atLimit ? 'warn' : DOT[h.state.tone],
      ...(h.logo && { logo: h.logo }),
    });
  }
  return out;
}

/** The item whose detail shows: the one picked while the list has it, else the list's first (the pick itself is kept,
 *  so clearing the search brings it back). */
export function shownItem(items: readonly NavItem[], picked: string): string | undefined {
  return items.some((i) => i.id === picked) ? picked : items[0]?.id;
}

/** What a harness's default model row says is wrong: a model it no longer offers (`models`: its ids, Default '' aside). */
export function modelProblem(model: string, models: readonly { readonly id: string }[], checking: boolean): 'unknown' | undefined {
  if (!model || checking || models.length === 0) return undefined;
  return models.some((m) => m.id === model) ? undefined : 'unknown';
}
