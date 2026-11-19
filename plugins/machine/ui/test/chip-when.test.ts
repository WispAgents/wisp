// ADR-0105: Home's gate (ChipGate.tsx) loads the chip only when it has something to show; it decides with chipWhen,
// the same rule the chip's words (view.ts chipOf) use, so the gate and the chip never disagree.
import { expect, test } from 'vitest';
import { chipInput, chipWhen } from '../src/chip-when.ts';
import { chipOf } from '../src/view.ts';
import type { MachineLoad } from '../src/types.ts';

const now = 1_000_000;
const load = (o: Partial<MachineLoad> = {}): MachineLoad => ({ at: now, mem_free_pct: 60, runs: 0, held: null, limits: { floor: 15 }, paused_until: 0, left_behind: 0, ...o });
const t = (k: string): string => k;

test('the_gate_shows_the_chip_exactly_when_the_chip_has_words', () => {
  const cases: [string, MachineLoad | undefined, number, boolean][] = [
    ['nothing read yet', undefined, 0, false],
    ['plenty of memory', load(), 0, false],
    ['memory near the floor', load({ mem_free_pct: 20 }), 0, true],
    ['work held', load({ held: { kind: 'memory', why: 'low' } }), 0, true],
    ['held only by a pause is the pause', load({ held: { kind: 'paused', why: '' } }), 0, false],
    ['automations paused', load({ paused_until: now + 60_000 }), 0, true],
    ['a pause that ended', load({ paused_until: now - 1 }), 0, false],
    ['processes left behind', load({ left_behind: 2 }), 0, true],
    ['left behind from the snapshot', load(), 1, true],
  ];
  for (const [what, l, left, shows] of cases) {
    const input = { load: l, pausedUntil: 0, left, now };
    expect(chipWhen(input) !== null, what).toBe(shows);
    expect(chipOf({ ...input, t }) !== null, what).toBe(shows);
  }
});

test('the_gate_reads_the_snapshot_when_there_is_no_load_row', () => {
  const snap = { at: now, mem: { freePct: 12 }, guard: { runs: 1, held: null, floor: 15, pausedUntil: 0 }, leftovers: [] } as never;
  expect(chipWhen(chipInput(undefined, snap, now))).not.toBeNull();
  expect(chipWhen(chipInput(load(), snap, now))).toBeNull();
});
