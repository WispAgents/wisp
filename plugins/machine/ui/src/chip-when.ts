// When the top-bar chip shows (design/machine-page.md "Where"): only when it matters. Pure and small, apart from its
// words (view.ts chipOf), so the gate on Home (ChipGate.tsx) decides without loading the chip (ADR-0105).
import type { MachineLoad, MachineSnapshot } from './types.ts';

/** the chip shows once free memory is under the floor plus this */
export const TIGHT = 10;
/** wisp's default memory floor (v1 server/load.js LOAD_DEFAULTS) */
export const FLOOR_DEFAULT = 15;

export interface ChipInput {
  readonly load: MachineLoad | undefined;
  readonly pausedUntil: number;
  readonly left: number;
  readonly now: number;
}

/** What the chip is about: memory tight, work held, automations paused, something left behind; null when none. */
export function chipWhen(o: ChipInput): { free: number | null; floor: number; tight: boolean; held: { readonly kind: string; readonly why: string } | null; until: number; paused: boolean; left: number } | null {
  const { load, now } = o;
  const free = load?.mem_free_pct ?? null;
  const floor = load?.limits.floor ?? FLOOR_DEFAULT;
  const held = load?.held && load.held.kind !== 'paused' ? load.held : null;
  const until = Math.max(o.pausedUntil, load?.paused_until ?? 0);
  const tight = free !== null && free < floor + TIGHT;
  const paused = until > now;
  const left = o.left || load?.left_behind || 0;
  return tight || held || paused || left ? { free, floor, tight, held, until, paused, left } : null;
}

/** The load the chip reads, from the snapshot when that's all there is. */
export function loadOf(m: MachineSnapshot): MachineLoad {
  const g = m.guard;
  return {
    at: m.at,
    ...(m.platform !== undefined && { platform: m.platform }),
    mem_free_pct: m.mem.freePct,
    runs: g.runs,
    held: g.held && g.held !== 'paused' ? { kind: g.held, why: g.why ?? '' } : null,
    limits: { floor: g.floor },
    paused_until: g.pausedUntil,
    left_behind: m.leftovers.length,
  };
}

/** The chip's input from what the peer sent: the cheap load row, else the snapshot's. */
export function chipInput(load: MachineLoad | undefined, snap: MachineSnapshot | undefined, now: number): ChipInput {
  return { load: load ?? (snap ? loadOf(snap) : undefined), pausedUntil: snap?.guard.pausedUntil ?? 0, left: snap?.leftovers.length ?? 0, now };
}
