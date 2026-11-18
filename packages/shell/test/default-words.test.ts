// Owner's delegate 2026-11-18: Default names the model it will run where the task runs: the picked project's pin
// (runtime.defaultModel reads the harness's config there), else this computer's (the harnesses topic), else "Default".
// Asked once per computer, harness and folder; a new folder asks again.
import { expect, test } from 'vitest';
import { createRoot, createSignal } from 'solid-js';
import { createClient, peerId, type Client } from '@wisp/client';
import { useDefaultWords } from '../src/frame/runner.ts';

/** until `ok`, to a clock deadline (the asking is its own chunk, then a request) */
async function until(ok: () => boolean, ms = 4000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}

test('default_names_the_projects_pinned_model_then_this_computers_and_asks_again_for_a_new_folder', async () => {
  const real = createClient({ clientId: 'c1', build: 't', home: peerId('local'), width: 1280 });
  const asked: string[] = [];
  const client = {
    scope: real.scope,
    peers: { ...real.peers, watch: () => undefined },
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

test('a_failed_ask_is_not_an_answer_the_next_look_asks_again', async () => {
  const real = createClient({ clientId: 'c2', build: 't', home: peerId('local'), width: 1280 });
  let calls = 0;
  const client = {
    scope: real.scope,
    peers: { ...real.peers, watch: () => undefined },
    entities: real.entities,
    commands: {
      run: async () => {
        calls += 1;
        if (calls === 1) throw new Error('asleep');
        return 'Opus';
      },
    },
  } as unknown as Client;
  const words = createRoot(() => useDefaultWords(client, () => 'local', () => '/code/app'));
  expect(words('claude', 'Default')).toBe('Default');
  await until(() => calls === 1);
  // the failure settles, then the next look asks again and names it
  await until(() => (words('claude', 'Default'), calls === 2));
  await until(() => words('claude', 'Default') === 'Opus');
  expect(words('claude', 'Default')).toBe('Opus');
  expect(calls).toBe(2);
});
