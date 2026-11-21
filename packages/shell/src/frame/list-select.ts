// The phone Tasks list's selection mode (ADR-0065 step 13, design/mobile.md §A.4): entered from a row's long-press
// menu (Select, first) or the bar's Select; a tap toggles a row; Select all picks every task the list shows (again:
// none); Back / × / Done leaves (and on Android, unpicking the last row: the kit's SelectionBar asks for that, as M3's
// contextual bar does). Rows that leave the list meanwhile (deleted, archived, filtered out) leave the selection.
// Pure: the shell's state calls these.

export interface ListSelection {
  readonly on: boolean;
  /** in the order they were picked */
  readonly keys: readonly string[];
}

export const NONE: ListSelection = { on: false, keys: [] };

/** Select (a row's menu: that row picked; the bar's: none yet). */
export const startSelect = (key?: string): ListSelection => ({ on: true, keys: key === undefined ? [] : [key] });

export function toggleSelect(s: ListSelection, key: string): ListSelection {
  if (!s.on) return s;
  return { on: true, keys: s.keys.includes(key) ? s.keys.filter((k) => k !== key) : [...s.keys, key] };
}

/** Every task the list shows, or none when they all are already (iOS's Deselect All; Android's Select all again). */
export function selectAll(s: ListSelection, order: readonly string[]): ListSelection {
  if (!s.on) return s;
  return { on: true, keys: allSelected(s, order) ? [] : [...order] };
}

/** Every task the list shows is picked (one pass over a set: 5k rows stay cheap). */
export function allSelected(s: ListSelection, order: readonly string[]): boolean {
  if (!s.on || order.length === 0 || s.keys.length < order.length) return false;
  const picked = new Set(s.keys);
  return order.every((k) => picked.has(k));
}

/** Drops the picked rows the list no longer shows. */
export function pruneSelect(s: ListSelection, order: readonly string[]): ListSelection {
  if (!s.on || s.keys.length === 0) return s;
  const shown = new Set(order);
  const keys = s.keys.filter((k) => shown.has(k));
  return keys.length === s.keys.length ? s : { on: true, keys };
}
