// Settings → Cloud → API keys (ADR-0058, v1 web/js/features/api-keys.js), DOM-free: what `grid-client.keys` lists,
// the rows as they change at once (a new key's row before the grid answers, a revoked one gone), and each row's
// line. A new key itself is never in here: the pane holds it in one local signal, shown once, gone on Done or when
// the pane is left. Never in the entity store, drafts, settings or storage.

export interface ApiKey {
  readonly id: string;
  readonly name: string;
  /** the key's last four characters */
  readonly hint: string;
  readonly createdAt: number;
  readonly lastUsedAt: number | null;
  /** made here, the grid hasn't answered yet */
  readonly pending?: boolean;
}

export interface KeysView {
  readonly keys: readonly ApiKey[];
  /** the cloud wisp's address, where a key is sent */
  readonly address: string;
}

/** The key just made, until Done: its name and the key. */
export interface Fresh {
  readonly name: string;
  readonly key: string;
}

type T = (k: string, vars?: Readonly<Record<string, string | number>>) => string;

export const NAME_MAX = 60;

const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);

/** One key as shown: only these fields, whatever else came (never a `key`). */
function keyOf(v: unknown): ApiKey | undefined {
  if (!v || typeof v !== 'object') return undefined;
  const o = v as Record<string, unknown>;
  if (typeof o['id'] !== 'string' || typeof o['name'] !== 'string') return undefined;
  return { id: o['id'], name: o['name'], hint: typeof o['hint'] === 'string' ? o['hint'] : '', createdAt: num(o['createdAt']) ?? 0, lastUsedAt: num(o['lastUsedAt']) };
}

/** `grid-client.keys`'s answer as rows, newest first; anything else is none. */
export function keysOf(v: unknown): KeysView {
  const o = v && typeof v === 'object' ? (v as Record<string, unknown>) : {};
  const list = Array.isArray(o['keys']) ? o['keys'] : [];
  return { keys: newest(list.map(keyOf).filter((k): k is ApiKey => k !== undefined)), address: typeof o['address'] === 'string' ? o['address'] : '' };
}

/** `grid-client.keysCreate`'s answer: the row and the key, or undefined when it has no key. */
export function madeOf(v: unknown): { readonly row: ApiKey; readonly key: string } | undefined {
  const row = keyOf(v);
  const key = (v as { key?: unknown } | null)?.key;
  return row && typeof key === 'string' && key.startsWith('wgk_') ? { row, key } : undefined;
}

const newest = (list: readonly ApiKey[]): ApiKey[] => [...list].sort((a, b) => b.createdAt - a.createdAt);

/** Why a name can't be sent (the grid's rules), or undefined. */
export function nameProblem(name: string): 'grid-client.keys.nameNeeded' | 'grid-client.keys.nameLong' | undefined {
  const n = name.trim();
  if (!n) return 'grid-client.keys.nameNeeded';
  if ([...n].length > NAME_MAX) return 'grid-client.keys.nameLong';
  return undefined;
}

/** Create, at once: the new row first, pending, with an id of its own until the grid's comes. */
export const adding = (list: readonly ApiKey[], tmp: string, name: string, now: number): ApiKey[] => [
  { id: tmp, name: name.trim(), hint: '', createdAt: now, lastUsedAt: null, pending: true },
  ...list,
];

/** The grid made it: its row where the pending one was (or none, when the grid refused). */
export const settled = (list: readonly ApiKey[], tmp: string, row: ApiKey | undefined): ApiKey[] =>
  row ? list.map((k) => (k.id === tmp ? row : k)) : list.filter((k) => k.id !== tmp);

/** Revoke, at once: the row goes. */
export const without = (list: readonly ApiKey[], id: string): ApiKey[] => list.filter((k) => k.id !== id);

/** The grid refused a revoke: the row comes back where it was. */
export const restored = (list: readonly ApiKey[], k: ApiKey): ApiKey[] => (list.some((x) => x.id === k.id) ? [...list] : newest([...list, k]));

/** The line under a key's name: `…Zq9x · Created 3m · Last used never` (`when` reads a past time). */
export function keyLine(k: ApiKey, when: (at: number) => string, t: T): string {
  return [
    k.pending ? t('grid-client.keys.making') : `…${k.hint}`,
    t('grid-client.keys.created', { when: when(k.createdAt) }),
    k.lastUsedAt ? t('grid-client.keys.used', { when: when(k.lastUsedAt) }) : t('grid-client.keys.neverUsed'),
  ].join(' · ');
}

/** How to try a key, with the key in `$WISP_KEY` (never the key itself: Copy is the one way it leaves the pane). */
export const tryLine = (address: string): string => `curl -H "Authorization: Bearer $WISP_KEY" ${address.replace(/\/$/, '')}/api/health`;
