// A task page's events by turn, once per change of the events held (perf r2 #5). Each turn filtered and sorted every
// event itself, on every change: a task's first snapshot, and each streamed event after it, ran that once per turn
// (turns × events), all inside the open's frames. Grouped once, a turn whose events didn't change keeps the same array,
// so only the turn a new event belongs to recomputes its steps.

/** What grouping reads of an event: its turn and its place in the task. */
export interface TurnEvent {
  readonly turn_id: string;
  readonly seq: number;
}

/** No events: one array for every turn without any, so they never look changed. */
export const NO_EVENTS: readonly never[] = Object.freeze([]);

const same = <T>(a: readonly T[], b: readonly T[]): boolean => a.length === b.length && a.every((x, i) => x === b[i]);

/** Flattens entity store `'event'` rows (individual snapshot rows and live `ChangeOp::Append` arrays). */
export function flattenEvents<T extends TurnEvent>(events: readonly (T | readonly T[])[]): T[] {
  const out: T[] = [];
  for (const item of events) {
    if (Array.isArray(item)) {
      for (const e of item as readonly T[]) if (e && typeof e.turn_id === 'string') out.push(e);
    } else if (item && typeof (item as T).turn_id === 'string') {
      out.push(item as T);
    }
  }
  return out;
}

/** `events` by turn, each oldest first (deduped by seq); a turn's array from `prev` when it holds the same events. */
export function eventsByTurn<T extends TurnEvent>(
  events: readonly (T | readonly T[])[],
  prev?: ReadonlyMap<string, readonly T[]>,
): Map<string, readonly T[]> {
  const by = new Map<string, Map<number, T>>();
  for (const e of flattenEvents(events)) {
    let seen = by.get(e.turn_id);
    if (!seen) by.set(e.turn_id, (seen = new Map()));
    seen.set(e.seq, e);
  }
  const out = new Map<string, readonly T[]>();
  for (const [turn, seen] of by) {
    const list = [...seen.values()].sort((a, b) => a.seq - b.seq);
    const was = prev?.get(turn);
    out.set(turn, was && same(was, list) ? was : list);
  }
  return out;
}
