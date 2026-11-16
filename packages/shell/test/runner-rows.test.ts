// The harness picker's rows from the harnesses topic (ADR-0066) follow each row as its detect settles (round 324): a
// createMemo over the keys read each row once, so a harness that settled after its key showed stayed Checking….
import { expect, test } from 'vitest';
import { createRoot } from 'solid-js';
import { signal, type EntityKey } from '@wisp/client';
import { HARNESS_ROW, useHarnessRows, type HarnessTopicRow } from '../src/frame/runner.ts';

const row = (id: string, state: HarnessTopicRow['state'], name = id): HarnessTopicRow => ({ id, ord: 0, state, agent: { id, name, available: state === 'installed', models: [], efforts: [] } });

test('the_picker_rows_follow_a_harness_that_settles_after_its_key_showed_one_renamed_and_one_deleted', () => {
  const keys = signal<readonly EntityKey[]>([]);
  const rows = new Map<string, ReturnType<typeof signal<HarnessTopicRow | undefined>>>();
  const at = (k: string) => rows.get(k) ?? (rows.set(k, signal<HarnessTopicRow | undefined>(undefined)), rows.get(k)!);
  const entities = {
    keys: (kind: string) => (kind === HARNESS_ROW ? keys : signal<readonly EntityKey[]>([])),
    row: (_kind: string, k: EntityKey) => at(k),
  };
  createRoot((dispose) => {
    const list = useHarnessRows(entities as never, () => 'laptop');
    const states = (): string[] => list().map((r) => `${r.agent.name}:${r.state}`);
    // every harness at once as checking, then another computer's (not this peer's)
    at('laptop:claude')(row('claude', 'checking', 'Claude Code'));
    at('box:codex')(row('codex', 'installed'));
    keys(['laptop:claude', 'box:codex'] as EntityKey[]);
    expect(states()).toEqual(['Claude Code:checking']);
    // its detect lands: same key, the row changes in place
    at('laptop:claude')(row('claude', 'installed', 'Claude Code'));
    expect(states(), 'settled').toEqual(['Claude Code:installed']);
    at('laptop:claude')(row('claude', 'installed', 'Claude Code 2'));
    expect(states(), 'renamed').toEqual(['Claude Code 2:installed']);
    // the row goes before its key does: nothing left of it
    at('laptop:claude')(undefined);
    expect(states(), 'deleted').toEqual([]);
    dispose();
  });
});
