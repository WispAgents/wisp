// The command palette's ranking (v1 web/js/fuzzy.js, tags.js, palette.js `results`), pure:
//   - each query word must appear in order (a subsequence) in the item; word starts, runs and early
//     positions score higher, and a plain substring on a word boundary beats any scattered match;
//   - the label is the field that's highlighted and counts 1.5×; sub, tags, keywords, group match too;
//   - "#auth" keeps only items tagged auth; a plain word that is a tag counts more than one in a title;
//   - recency: +20 just now, +10 a day old, never 0, so newer wins between equal matches;
//   - best first, groups kept together in the order of their best hit, 12 per group;
//   - an empty query shows each group's first `limit` (6) items, in the order given.
// Per key it does the least (perf-ui-r2 #2): each item's fields are lowercased once per list (`createRanker`), the
// query is parsed once, and a query that extends the last one only ranks the last one's hits.

import type { PaletteItem } from '../contract.ts';

const BOUNDARY = /[\s\-_./:#@()[\]]/;
/** what a substring on a word start adds over the same letters scattered */
const WORD = 10;

function isStart(text: string, i: number): boolean {
  if (i === 0) return true;
  const prev = text[i - 1] ?? '';
  const cur = text[i] ?? '';
  return BOUNDARY.test(prev) || (prev === prev.toLowerCase() && cur !== cur.toLowerCase());
}

function score(text: string, idx: readonly number[]): number {
  let s = 0;
  for (let k = 0; k < idx.length; k++) {
    const i = idx[k] ?? 0;
    s += 1;
    if (isStart(text, i)) s += k === 0 ? 8 : 5;
    if (k > 0) {
      const gap = i - (idx[k - 1] ?? 0) - 1;
      s += gap === 0 ? 4 : -Math.min(gap, 6) * 0.6;
    }
  }
  if (idx[0] === 0) s += 6;
  return s - (idx[0] ?? 0) * 0.15 - text.length * 0.02;
}

export interface Hit {
  score: number;
  idx: number[];
}

/** One needle against one text, or null. */
export function matchOne(needle: string, text: string): Hit | null {
  return matchLow(needle.toLowerCase(), text, text.toLowerCase());
}

/** `matchOne` with the needle (`n`) and the text (`h`) already lower case; `text` as written, for word starts. */
function matchLow(n: string, text: string, h: string): Hit | null {
  if (!n) return { score: 0, idx: [] };
  let end = -1;
  for (let i = 0, j = 0; i < h.length; i++) {
    if (h[i] === n[j] && ++j === n.length) {
      end = i;
      break;
    }
  }
  if (end < 0) return null;
  const sub = h.indexOf(n);
  if (sub >= 0) {
    let at = sub;
    let word = false;
    for (let k = sub; k >= 0; k = h.indexOf(n, k + 1)) {
      if (isStart(text, k)) {
        at = k;
        word = true;
        break;
      }
    }
    const idx = Array.from(n, (_, k) => at + k);
    // the needle as a word (or a word's start) beats letters scattered anywhere: deep in a long lede ("next to the
    // model in the launcher") its position cost more than the scattered "mode … colors" did (audit C7)
    return { score: score(text, idx) + (word ? WORD : 0), idx };
  }
  let start = end;
  for (let j = n.length - 1; j >= 0; start--) if (h[start] === n[j]) j--;
  start++;
  const idx: number[] = [];
  for (let i = start, j = 0; j < n.length; i++) {
    if (h[i] === n[j]) {
      idx.push(i);
      j++;
    }
  }
  return { score: score(text, idx), idx };
}

/** A whole query against [label, ...rest]; idx are the label's matched positions. */
export function fuzzy(query: string, fields: readonly string[]): Hit | null {
  return fuzzyLow(words(query), fields, fields.map((f) => f.toLowerCase()));
}

const words = (q: string): string[] =>
  q
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => w.toLowerCase());

/** `fuzzy` with the query's words lower case and each field's lower case beside it (`low`). */
function fuzzyLow(ws: readonly string[], fields: readonly string[], low: readonly string[]): Hit | null {
  if (ws.length === 0) return { score: 0, idx: [] };
  let total = 0;
  const idx = new Set<number>();
  for (const w of ws) {
    const t = matchLow(w, fields[0] ?? '', low[0] ?? '');
    let best = t ? t.score * 1.5 : -Infinity;
    for (let i = 1; i < fields.length; i++) {
      const m = fields[i] ? matchLow(w, fields[i] ?? '', low[i] ?? '') : null;
      if (m && m.score > best) best = m.score;
    }
    if (best === -Infinity) return null;
    if (t && t.score * 1.5 === best) for (const i of t.idx) idx.add(i);
    total += best;
  }
  return { score: total, idx: [...idx].sort((a, b) => a - b) };
}

export function recency(at: number | undefined, now: number): number {
  if (!at) return 0;
  const hours = Math.max(0, now - at) / 3.6e6;
  return 20 / (1 + hours / 24);
}

export interface Query {
  tags: string[];
  text: string;
}

export function splitQuery(q: string): Query {
  const tags: string[] = [];
  const words: string[] = [];
  for (const w of q.trim().split(/\s+/).filter(Boolean)) {
    if (w.startsWith('#') && w.length > 1) tags.push(w.slice(1).toLowerCase());
    else if (w !== '#') words.push(w);
  }
  return { tags, text: words.join(' ') };
}

export function tagScore(q: Query, tags: readonly string[] = []): { score: number; hit: string[] } | null {
  return tagScoreOf(q, words(q.text), tags);
}

function tagScoreOf(q: Query, textWords: readonly string[], tags: readonly string[] = []): { score: number; hit: string[] } | null {
  const hit = new Set<string>();
  let s = 0;
  for (const w of q.tags) {
    const t = tags.find((x) => x === w) ?? tags.find((x) => x.startsWith(w));
    if (t === undefined) return null;
    hit.add(t);
    s += t === w ? 14 : 8;
  }
  for (const w of textWords) {
    const t = tags.find((x) => x === w) ?? (w.length > 2 ? tags.find((x) => x.startsWith(w)) : undefined);
    if (t !== undefined) {
      hit.add(t);
      s += t === w ? 10 : 5;
    }
  }
  return { score: s, hit: [...hit] };
}

export interface Ranked {
  readonly item: PaletteItem;
  /** the label's matched positions, to mark */
  readonly idx: readonly number[];
}

const GROUP_LIMIT = 6;
const PER_GROUP = 12;

export function rank(items: readonly PaletteItem[], query: string, now: number, deep: readonly PaletteItem[] = []): Ranked[] {
  return createRanker()(items, query, now, deep);
}

/** An item's searched fields (label, sub, tags, keywords, group) as written and lower case, made once per list. */
interface Fields {
  readonly raw: readonly string[];
  readonly low: readonly string[];
}

const fieldsOf = (it: PaletteItem): Fields => {
  const raw = [it.label, it.sub ?? '', it.tags?.join(' ') ?? '', it.keywords ?? '', it.group];
  return { raw, low: raw.map((f) => f.toLowerCase()) };
};

export interface Ranker {
  (items: readonly PaletteItem[], query: string, now: number, deep?: readonly PaletteItem[]): Ranked[];
  /** how many items the last call scored (a test's measure of the work per key) */
  readonly scored: number;
}

/**
 * `rank` that keeps what it can between calls on the same `items` (the palette's, one per open): each item's fields
 * lowercased once, and the last query's hits, so a query that extends it ("au" → "aut", "fix" → "fix au") only ranks
 * those: every word or `#tag` it adds can only narrow them. A new `items` list starts again.
 */
export function createRanker(): Ranker {
  // by item id: its fields as last seen (lowercased once), and whether the last query matched it. Kept across a new
  // list (the store changed while the palette is open: a task streaming rebuilds the task items each patch), so a
  // rebuild costs neither the lowercasing nor the narrowing; an item whose fields changed is ranked again.
  let seen = new Map<string, Fields>();
  let last: { q: string; hits: ReadonlySet<string> } | undefined;
  let scored = 0;
  const fieldsAt = (it: PaletteItem): Fields => {
    const was = seen.get(it.id);
    if (was && sameFields(was, it)) return was;
    const f = fieldsOf(it);
    seen.set(it.id, f);
    return f;
  };
  const run = (items: readonly PaletteItem[], query: string, now: number, deep: readonly PaletteItem[] = []): Ranked[] => {
    const qt = query.trim();
    if (!qt) {
      last = undefined;
      scored = 0;
      const per = new Map<string, number>();
      return items
        .filter((it) => {
          const n = per.get(it.group) ?? 0;
          per.set(it.group, n + 1);
          return n < GROUP_LIMIT;
        })
        .map((item) => ({ item, idx: [] }));
    }
    if (seen.size > items.length * 2 + 64) seen = new Map(); // items gone for good: don't keep them
    const q = splitQuery(query);
    const textWords = words(q.text);
    const narrow = last && qt.startsWith(last.q) ? last.hits : undefined;
    const kept = new Set<string>();
    const hits: { item: PaletteItem; idx: number[]; score: number }[] = [];
    let n = 0;
    for (const it of items) {
      // the last query's hits, and any item new or changed since it ran
      if (narrow && !narrow.has(it.id)) {
        const was = seen.get(it.id);
        if (was && sameFields(was, it)) continue;
      }
      n++;
      const t = tagScoreOf(q, textWords, it.tags);
      if (!t) continue;
      if (it.fill !== undefined && it.tags?.[0] !== undefined && q.tags.includes(it.tags[0])) {
        kept.add(it.id); // that topic is already in the query; "#tag" → "#tags" can bring it back, so it stays a candidate
        continue;
      }
      const f = fieldsAt(it);
      const m: Hit | null = textWords.length > 0 ? fuzzyLow(textWords, f.raw, f.low) : { score: 0, idx: [] };
      if (!m) continue;
      kept.add(it.id);
      hits.push({ item: it, idx: m.idx, score: m.score + t.score + (it.rank ?? 0) + recency(it.at, now) });
    }
    scored = n;
    last = { q: qt, hits: kept };
    return grouped(hits, deep);
  };
  return Object.defineProperty(run, 'scored', { get: () => scored }) as Ranker;
}

/** Whether `it` still has the fields `f` was made from (a renamed task, its new state in `sub`, a tag added: no). */
function sameFields(f: Fields, it: PaletteItem): boolean {
  const r = f.raw;
  return r[0] === it.label && r[1] === (it.sub ?? '') && r[3] === (it.keywords ?? '') && r[4] === it.group && r[2] === (it.tags?.join(' ') ?? '');
}

function grouped(hits: { item: PaletteItem; idx: number[]; score: number }[], deep: readonly PaletteItem[]): Ranked[] {
  hits.sort((a, b) => b.score - a.score || (b.item.at ?? 0) - (a.item.at ?? 0));
  const groups = [...new Set(hits.map((h) => h.item.group))];
  const listed = new Set(hits.map((h) => h.item.id));
  const extra = deep.filter((it) => !listed.has(it.id)).map((item) => ({ item, idx: [] as number[] }));
  return [...groups.flatMap((g) => hits.filter((h) => h.item.group === g).slice(0, PER_GROUP)), ...extra];
}

/** Splits a label into runs for <mark>: [text, marked][] */
export function marks(text: string, idx: readonly number[]): [string, boolean][] {
  if (idx.length === 0) return [[text, false]];
  const on = new Set(idx);
  const out: [string, boolean][] = [];
  for (let i = 0; i < text.length; i++) {
    const hit = on.has(i);
    const last = out.at(-1);
    if (last && last[1] === hit) last[0] += text[i] ?? '';
    else out.push([text[i] ?? '', hit]);
  }
  return out;
}
