// perf r1 #15: a stream patches one task's row many times a second; the list's rows are made once per server row, so a
// patch makes that one row again and every other row stays the same object (no 5,000-row rebuild per token).
import { expect, test, vi } from 'vitest';
import * as wireMod from '../src/model/wire.ts';
import { rowMaker } from '../src/state.ts';

vi.mock('../src/model/wire.ts', async (real) => {
  const m = await real<typeof import('../src/model/wire.ts')>();
  return { ...m, flatSession: vi.fn(m.flatSession) };
});
const flats = (): number => vi.mocked(wireMod.flatSession).mock.calls.length;
import type { MachineRef, SessionRow } from '../src/types.ts';

const wire = (id: string, preview: string): SessionRow =>
  ({ id, title: id, state: 'running', agent: 'claude', created_at: 1, updated_at: 2, body: { cwd: '/w' }, summary: { status: 'running', preview } }) as unknown as SessionRow;

test('a_patch_to_one_task_makes_only_its_row_again_and_a_row_under_a_new_key_is_made_for_it', () => {
  const make = rowMaker();
  const ps: never[] = [];
  const ms = new Map<string, MachineRef>();
  const raw = [wire('a', ''), wire('b', ''), wire('c', '')];
  const keys = ['local:a', 'local:b', 'local:c'];
  const first = make(raw, keys, ps, ms);
  expect(first.map((r) => r.id)).toEqual(['a', 'b', 'c']);

  // the store swaps b's object for its patched copy and keeps a and c
  const patched = [raw[0]!, wire('b', 'hello'), raw[2]!];
  const before = flats();
  const second = make(patched, keys, ps, ms);
  expect(flats() - before).toBe(1);
  expect(second[0]).toBe(first[0]);
  expect(second[2]).toBe(first[2]);
  expect(second[1]).not.toBe(first[1]);
  expect((second[1] as unknown as { preview: string }).preview).toBe('hello');

  // the same server row under another key (another wisp's) is made for that key; the rest come out the same objects
  // (the flat, project and machine steps keep theirs)
  const moved = make(patched, ['local:a', 'mac:b', 'local:c'], ps, new Map([['mac', { id: 'm1', kind: 'ssh', name: 'Mac' } satisfies MachineRef]]));
  expect(moved[1]?.machine?.name).toBe('Mac');
  expect(moved[0]).toBe(second[0]);
  expect(moved[0]?.id).toBe('a');
});
