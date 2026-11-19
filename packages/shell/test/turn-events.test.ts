// perf r2 #5: a task page groups its events by turn once per change, and a turn whose events didn't change keeps its
// array (so only the turn a streamed event belongs to recomputes its steps).
import { describe, expect, it } from 'vitest';
import { NO_EVENTS, eventsByTurn } from '../src/frame/turn-events.ts';

const ev = (turn_id: string, seq: number): { turn_id: string; seq: number } => ({ turn_id, seq });

describe('eventsByTurn', () => {
  it('groups by turn, each oldest first', () => {
    const a1 = ev('a', 3);
    const a0 = ev('a', 1);
    const b0 = ev('b', 2);
    const by = eventsByTurn([a1, b0, a0]);
    expect(by.get('a')).toEqual([a0, a1]);
    expect(by.get('b')).toEqual([b0]);
    expect(by.get('c') ?? NO_EVENTS).toBe(NO_EVENTS);
  });

  it('keeps a turn\'s array when its events are the same, and makes a new one for the turn that changed', () => {
    const a0 = ev('a', 1);
    const b0 = ev('b', 2);
    const first = eventsByTurn([a0, b0]);
    const b1 = ev('b', 3);
    const next = eventsByTurn([a0, b0, b1], first);
    expect(next.get('a')).toBe(first.get('a'));
    expect(next.get('b')).not.toBe(first.get('b'));
    expect(next.get('b')).toEqual([b0, b1]);
    // an event replaced by a newer copy (an updated row) is a change
    const a0b = ev('a', 1);
    expect(eventsByTurn([a0b, b0, b1], next).get('a')).not.toBe(next.get('a'));
  });
});
