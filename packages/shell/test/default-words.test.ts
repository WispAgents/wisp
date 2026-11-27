// Owner's delegate 2026-11-18: Default names the model it will run where the task runs: the picked project's pin
// (runtime.defaultModel reads the harness's config there), else this computer's (the harnesses topic), else "Default".
// Asked once per computer, harness and folder; a new folder asks again.
import { expect, test } from 'vitest';
import { createRoot, createSignal } from 'solid-js';
import { createClient, peerId, type Client } from '@wisp/client';
import { useDefaultWords } from '../src/frame/runner.ts';

/** until `ok`, to a clock deadline (the asking is its own chunk, then a request) */
async function until(ok: () => boolean, ms = 15_000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}

test('default_names_the_projects_pinned_model_then_this_computers_and_asks_again_for_a_new_folder', async () => {
  const real = createClient({ clientId: 'c1', build: 't', home: peerId('local'), width: 1280 });
  const asked: string[] = [];
  const client = {
    scope: real.scope,
    peers: { ...real.peers, watch: () => undefined, status: () => () => ({ kind: 'online' }) },
    entities: real.entities,
    commands: {
      run: async (cmd: { id: string }, a: { agent: string; cwd?: string }) => {
        asked.push(`${cmd.id} ${a.agent} ${a.cwd ?? '-'}`);
        return a.agent === 'claude' && a.cwd === '/code/app' ? 'Opus' : null;
      },
    },
  } as unknown as Client;
  const [cwd, setCwd] = createSignal<string | undefined>('/code/app');
  const words = createRoot(() => useDefaultWords(client, () => 'local', cwd));
  // asked, then named
  expect(words('claude', 'Default')).toBe('Default');
  await until(() => words('claude', 'Default') === 'Opus');
  expect(words('claude', 'Default')).toBe('Opus');
  // asked once for that folder, however often it's read
  words('claude', 'Default');
  expect(asked.filter((a) => a === 'runtime.defaultModel claude /code/app')).toHaveLength(1);
  // a harness that can't tell there: Default
  expect(words('codex', 'Default')).toBe('Default');
  await until(() => asked.includes('runtime.defaultModel codex /code/app'));
  expect(words('codex', 'Default')).toBe('Default');
  // another folder: asked again, and this computer's (none here): Default
  setCwd('/code/other');
  expect(words('claude', 'Default')).toBe('Default');
  await until(() => asked.includes('runtime.defaultModel claude /code/other'));
  expect(asked).toContain('runtime.defaultModel claude /code/other');
  expect(words('claude', 'Default')).toBe('Default');
});
// a failed ask's retry waits a minute: default-models.test.ts
