// Each task keeps its place (v1 split.js keepPlace / backToPlace, F-task-17): where it was scrolled when you left it,
// so going back, or a reload, lands on the same spot. Held by the turn at the top of the pane and how far into it, so a
// turn landing meanwhile or older ones folding away don't shift what was being read; at the end, back to the end, new
// turns and all. DOM-free: the box's numbers and the turns' tops come in.
import type { Json, PluginClient, PluginStore } from '@wisp/client';

export interface Place {
  readonly end: boolean;
  /** the turn at the top of the pane, and how far its top sits above the pane's top (px) */
  readonly turn?: string;
  readonly dy: number;
  /** the raw offset, when no turn can say */
  readonly top: number;
  /** whether the task's Transcript view (V) was opened on this task */
  readonly transcript?: boolean;
}

export interface Box {
  readonly scrollTop: number;
  readonly clientHeight: number;
  readonly scrollHeight: number;
}

export interface PlaceOptions {
  /** Whether the view is actively following the end (always recorded as `end: true`). */
  readonly following?: boolean;
  /** Chat mode opens at the end by default, so scrolling up to the very top of a long conversation is kept explicitly. */
  readonly chat?: boolean;
  /** Whether the task's Transcript view (V) is on (off by default in both Chat and Cards modes). */
  readonly transcript?: boolean;
}

/** The place to keep, or undefined at the default start (nothing to come back to). `turns`: each turn's top in the box's content. */
export function placeOf(box: Box, turns: readonly { readonly id: string; readonly top: number }[], opts?: PlaceOptions): Place | undefined {
  const scrollable = box.scrollHeight - box.clientHeight > 1;
  const end = opts?.following === true || box.scrollTop + box.clientHeight >= box.scrollHeight - 4;
  const tx = opts?.transcript === true ? { transcript: true as const } : {};
  if (box.scrollTop < 1) {
    // In Chat mode, scrolling all the way up to turn 1 on a scrollable conversation is an explicit reading position
    if (opts?.chat && scrollable && !end) {
      const at = turns[0];
      return { end: false, ...(at && { turn: at.id }), dy: 0, top: 0, ...tx };
    }
    return opts?.transcript === true ? { end: Boolean(opts?.chat || end), dy: 0, top: 0, transcript: true } : undefined;
  }
  // the last turn whose top is at or above the pane's top
  const at = [...turns].reverse().find((t) => t.top <= box.scrollTop + 1);
  return { end, ...(at && { turn: at.id }), dy: at ? box.scrollTop - at.top : 0, top: box.scrollTop, ...tx };
}

/** Where to scroll to on coming back. */
export function scrollFor(p: Place, box: Pick<Box, 'clientHeight' | 'scrollHeight'>, turns: readonly { readonly id: string; readonly top: number }[]): number {
  const max = Math.max(0, box.scrollHeight - box.clientHeight);
  if (p.end) return max;
  const t = p.turn !== undefined ? turns.find((x) => x.id === p.turn) : undefined;
  return Math.min(max, Math.max(0, t ? t.top + p.dy : p.top));
}

/** v1 MAX_PLACES: the most recent tasks' places are kept, the oldest go first. */
export const MAX_PLACES = 100;

/** The kept places with `id`'s set (newest last) or dropped, at most MAX_PLACES. */
export function keep(all: Readonly<Record<string, Place>>, id: string, p: Place | undefined): Record<string, Place> {
  const rest = Object.entries(all).filter(([k]) => k !== id);
  const next = p ? [...rest, [id, p] as const] : rest;
  return Object.fromEntries(next.slice(-MAX_PLACES));
}

const stores = new WeakMap<object, PluginStore<Json>>();
const loaded = new WeakSet<object>();

/** The kept places, in this browser (v1 localStorage wisp.places): the shell's client's store. */
export function placesStore(root: { plugin(id: string): PluginClient }): PluginStore<Json> {
  const had = stores.get(root);
  if (had) return had;
  const store = root
    .plugin('shell')
    .defineStore<Json>({ id: 'shell.places', version: 1, defaults: {}, persist: true, check: (v) => (v && typeof v === 'object' && !Array.isArray(v) ? (v as Json) : undefined) });
  stores.set(root, store);
  void store.ready.then(
    () => loaded.add(store),
    () => loaded.add(store),
  );
  return store;
}

/** True once the persisted places store has finished its initial load (so task switches restore without waiting a frame). */
export const placesLoaded = (store: PluginStore<Json>): boolean => loaded.has(store);
