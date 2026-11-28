// The filters an issue or review trigger's form offers (ADR-0043 `match`), as the CLI writes them
// (crates/wisp/src/trigger_cli.rs): whose (`me`: the GitHub login each computer's gh is signed in as), from whom
// (no bots, or only people who may push), its milestone, and for a review: drafts, the changed paths and the size.
// DOM-free. A filter a formula wrote that the form can't show is kept on an edit, never dropped.

/** Whose issues or reviews: anyone's (''), assigned to me, opened by me, or asked of me. */
export type Whose = '' | 'assigned' | 'opened' | 'asked';
/** Who it's from: anyone (''), people (no bots), or people who may push to the repo. */
export type From = '' | 'people' | 'trusted';
/** The most lines (added + deleted) a review may have ('' : any). */
export type Size = '' | '100' | '400' | '1000';

export interface Filters {
  readonly whose: Whose;
  readonly from: From;
  readonly milestone: string;
  /** a review: skip draft PRs */
  readonly skipDrafts: boolean;
  /** a review: changed files, as .gitignore globs, comma-separated (`src/**, !**\/*.md`) */
  readonly paths: string;
  readonly size: Size;
}

export const noFilters: Filters = { whose: '', from: '', milestone: '', skipDrafts: false, paths: '', size: '' };

/** What each kind's form offers. */
export const FILTERS_OF: Readonly<Record<string, { readonly whose: readonly Whose[]; readonly review: boolean }>> = {
  issues: { whose: ['', 'assigned', 'opened'], review: false },
  review: { whose: ['', 'asked'], review: true },
};
export const FROM: readonly From[] = ['', 'people', 'trusted'];
export const SIZES: readonly Size[] = ['', '100', '400', '1000'];

/** The `match` key each whose choice sets to `me`. */
const WHOSE_KEY = { assigned: 'assignees', opened: 'author', asked: 'reviewer' } as const;

const cut = (s: string): string => s.trim();
const list = (s: string): string[] => s.split(',').map(cut).filter(Boolean);

/** The `match` a kind's filters write ({} : none). A choice the kind doesn't offer writes nothing. */
export function matchOf(kind: string, f: Filters): Record<string, unknown> {
  const offer = FILTERS_OF[kind];
  if (!offer) return {};
  const m: Record<string, unknown> = {};
  if (f.whose && offer.whose.includes(f.whose)) m[WHOSE_KEY[f.whose]] = 'me';
  if (f.from === 'people') m['bot'] = false;
  if (f.from === 'trusted') m['trusted'] = true;
  if (cut(f.milestone)) m['milestone'] = cut(f.milestone);
  if (offer.review) {
    if (f.skipDrafts) m['draft'] = false;
    if (list(f.paths).length) m['paths'] = list(f.paths);
    if (f.size) m['size'] = `<=${f.size}`;
  }
  return m;
}

/** A saved trigger's `match` back as the form's filters, and the keys the form showed (an edit replaces only those). */
export function filtersOf(kind: string, match: Readonly<Record<string, unknown>>): { filters: Filters; owned: string[] } {
  const offer = FILTERS_OF[kind];
  if (!offer) return { filters: noFilters, owned: [] };
  const owned: string[] = [];
  const own = <T>(k: string, v: T): T => (owned.push(k), v);
  let whose: Whose = '';
  for (const w of offer.whose) {
    if (!w) continue;
    const k = WHOSE_KEY[w as Exclude<Whose, ''>];
    // `assignee: me` (the first assignee) reads as Assigned to me too
    const at = match[k] === 'me' ? k : w === 'assigned' && match['assignee'] === 'me' ? 'assignee' : undefined;
    if (at && !whose) whose = own(at, w);
  }
  const from: From = match['trusted'] === true ? own('trusted', 'trusted') : match['bot'] === false ? own('bot', 'people') : '';
  const milestone = typeof match['milestone'] === 'string' ? own('milestone', match['milestone']) : '';
  let skipDrafts = false;
  let paths = '';
  let size: Size = '';
  if (offer.review) {
    if (match['draft'] === false) skipDrafts = own('draft', true);
    const p = match['paths'];
    if (typeof p === 'string' || (Array.isArray(p) && p.every((x) => typeof x === 'string'))) paths = own('paths', (Array.isArray(p) ? p : [p]).join(', '));
    const n = typeof match['size'] === 'string' ? /^<=\s*(\d+)$/.exec(match['size'])?.[1] : undefined;
    const s = SIZES.find((x) => x && x === n);
    if (s) size = own('size', s);
  }
  return { filters: { whose, from, milestone, skipDrafts, paths, size }, owned };
}

/** The filters in a trigger's line: "my assigned ", " from people who may push", … */
export function filterWords(kind: string, match: Readonly<Record<string, unknown>>): { before: string; after: string } {
  const { filters: f } = filtersOf(kind, match);
  const before = f.whose === 'assigned' ? 'my assigned ' : f.whose === 'opened' ? 'my own ' : '';
  const bits = [
    f.whose === 'asked' ? 'for me' : '',
    f.from === 'people' ? 'not from bots' : f.from === 'trusted' ? 'from people who may push' : '',
    f.skipDrafts ? 'not drafts' : '',
    f.size ? `up to ${f.size} lines` : '',
    f.paths ? `touching ${f.paths}` : '',
    f.milestone ? `in ${f.milestone}` : '',
  ].filter(Boolean);
  return { before, after: bits.length ? ` ${bits.join(', ')}` : '' };
}
