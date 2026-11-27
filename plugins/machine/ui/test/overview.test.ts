// The Overview tab (ADR-0144, pick B): the status line says the worst thing first, the bars carry their wait marks,
// and a left-behind process is named by what it runs, not by its interpreter's path.
import { expect, test } from 'vitest';
import type { MachineSnapshot } from '../src/types.ts';
import { loads, shortCommand, statusOf } from '../src/view.ts';

const GB = 1024 ** 3;
const t = (k: string, v?: Readonly<Record<string, string | number>>): string => `${k}${v ? ` ${Object.values(v).join(' ')}` : ''}`;

function snap(o: { free?: number; mem?: number | null; cpu?: number; floor?: number | null; held?: string | null; paused?: number; left?: number; tasks?: number } = {}): MachineSnapshot {
  return {
    at: 1,
    mem: { freePct: o.mem === undefined ? 80 : o.mem, total: 24 * GB, per: 'footprint', pressure: null },
    cpu: { pct: o.cpu ?? 160, cores: 8 },
    disk: { free: o.free ?? 200 * GB, total: 1000 * GB, worktrees: 24 * GB, reclaimable: 0 },
    wisp: { rss: 1, cpu: 1 },
    guard: { floor: o.floor === undefined ? 15 : o.floor, ceiling: 200, maxRuns: 8, held: o.held ?? null, why: '', pausedUntil: o.paused ?? 0, battery: null },
    tasks: Array.from({ length: o.tasks ?? 0 }, (_, i) => ({ id: `t${i}`, title: 't', rss: 1, cpu: 1, procs: 1, worktreeBytes: null })),
    crew: [],
    leftovers: Array.from({ length: o.left ?? 0 }, (_, i) => ({ pid: i, command: 'x', rss: 1, since: 0, task: null })),
    leftover_groups: [],
    history: [],
    reclaim: [],
  } as unknown as MachineSnapshot;
}

test('the_status_says_the_worst_first_memory_then_disk_then_pause_wait_leftovers_then_room_to_spare', () => {
  expect(statusOf(snap({ mem: 9, free: 5 * GB }), 10, t as never).title).toBe('machine.status.memoryShort');
  expect(statusOf(snap({ free: 5 * GB }), 10, t as never)).toMatchObject({ tone: 'bad', title: 'machine.status.diskShort' });
  expect(statusOf(snap({ paused: 99 }), 10, t as never)).toMatchObject({ tone: 'warn', title: 'machine.status.paused' });
  const wait = statusOf(snap({ held: 'battery' }), 10, t as never);
  expect(wait.title).toBe('machine.status.waiting');
  expect(wait.detail.split(' · ')).toHaveLength(2);
  expect(statusOf(snap({ left: 2 }), 10, t as never).title).toBe('machine.status.leftovers 2');
  const ok = statusOf(snap({ tasks: 2 }), 10, t as never);
  expect(ok).toMatchObject({ tone: 'good', title: 'machine.status.ok' });
  expect(ok.detail).toContain('machine.status.runs 2');
});

test('the_bars_are_used_shares_with_the_wait_mark_where_automations_stop', () => {
  const [mem, cpu, disk] = loads(snap({ mem: 80, cpu: 160 }), t as never);
  expect(mem?.share).toBeCloseTo(0.2);
  expect(mem).toMatchObject({ mark: 0.85, sub: 'machine.load.memoryWait 85' });
  expect(cpu?.share).toBeCloseTo(0.2);
  expect(cpu).toMatchObject({ mark: 0.25, tone: 'accent' });
  expect(disk?.share).toBeCloseTo(0.8);
  expect(loads(snap({ cpu: 300 }), t as never)[1]?.tone).toBe('warn');
  expect(loads(snap({ floor: null }), t as never)[0]?.mark).toBeNull();
  // still reading: three placeholders of the same shape
  expect(loads(undefined, t as never).map((l) => l.share)).toEqual([undefined, undefined, undefined]);
});

test('a_left_behind_process_is_named_by_what_it_runs', () => {
  expect(shortCommand('/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/Resources/Python.app/Contents/MacOS/Python qa/parity.mjs --port 4590')).toBe('python3 qa/parity.mjs');
  expect(shortCommand('/usr/local/bin/node --inspect /Users/me/app/server.js')).toBe('node /Users/me/app/server.js');
  expect(shortCommand('/usr/bin/caffeinate -i')).toBe('caffeinate -i');
  expect(shortCommand('/opt/x/Python.app/Contents/MacOS/Python -m http.server 8765')).toBe('python3 -m http.server');
  expect(shortCommand('vite')).toBe('vite');
});
