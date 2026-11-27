// The Space tab's sums (ADR-0144): the disk bar's parts and the warning on top.
import { expect, test } from 'vitest';
import type { BuildGroup, MachineSnapshot } from '../src/types.ts';
import { alertOf, diskParts } from '../src/view.ts';

const GB = 1024 ** 3;
const t = (k: string, v?: Readonly<Record<string, string | number>>): string => `${k}${v ? ` ${Object.values(v).join(' ')}` : ''}`;

function snap(free: number, build: BuildGroup | null, floor: number | null = 15, mem = 50): MachineSnapshot {
  return {
    id: 'this',
    at: 1,
    platform: 'darwin',
    mem: { freePct: mem, total: 24 * GB, per: 'footprint', pressure: null },
    cpu: { pct: 10, cores: 8 },
    disk: { free, total: 1000 * GB, worktrees: 140 * GB, reclaimable: build?.bytes ?? 0 },
    guard: { floor, ceiling: 200 },
    reclaim: build ? [build] : [],
  } as unknown as MachineSnapshot;
}

const group: BuildGroup = {
  kind: 'build',
  count: 2,
  bytes: 125 * GB,
  ids: ['task-1'],
  leftovers: ['wisp-8fb6'],
  folders: [
    { id: 'wisp-8fb6', title: 'wisp-8fb6', bytes: 120 * GB, leftover: true, only: 580 },
    { id: 'task-1', title: 'Fix the list', bytes: 5 * GB, leftover: false },
  ],
};

test('the_disk_bar_is_build_output_then_wisps_other_folders_then_everything_else_and_the_track_is_free', () => {
  const parts = diskParts(snap(140 * GB, group), t as never);
  expect(parts.map((p) => [p.key, Math.round(p.value * 1000)])).toEqual([
    ['build', 125],
    ['wisp', 15],
    ['other', 720],
  ]);
  // no build output: the part isn't drawn
  expect(diskParts(snap(140 * GB, null), t as never).map((p) => p.key)).toEqual(['wisp', 'other']);
});

test('the_warning_says_the_disk_first_and_offers_the_build_output_then_memory_under_the_floor', () => {
  expect(alertOf(snap(8 * GB, group))).toEqual({ kind: 'disk', free: 8 * GB, build: 125 * GB });
  expect(alertOf(snap(8 * GB, null))).toEqual({ kind: 'disk', free: 8 * GB, build: 0 });
  expect(alertOf(snap(140 * GB, group))).toBeUndefined();
  expect(alertOf(snap(140 * GB, group, 15, 9))).toEqual({ kind: 'memory', free: 9 });
  expect(alertOf(snap(140 * GB, group, null, 9))).toBeUndefined();
});
