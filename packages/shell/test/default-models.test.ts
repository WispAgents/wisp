// One name for what Default runs, everywhere (owner 2026-11-18, QA r10 #2 follow-up): Home's chip and picker, the task's
// facts and reply chip, Compare, the sidebar card and the list row read one store per client; the folder in focus asks
// (Home's, the open task's), the rest read its answer (audit C #17). Each computer, harness and
// folder is asked once and shared; a computer that isn't online is never asked (no waking it, no queued command); a
// failed ask waits a minute before the next.
import { expect, test } from 'vitest';
import { createRoot, createSignal } from 'solid-js';
import type { Client } from '@wisp/client';
import { defaultModelName, defaultModels, runModelWord } from '../src/frame/default-models.ts';

async function until(ok: () => boolean, ms = 4000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}

function stub(o: { fail?: () => boolean } = {}) {
  const asked: string[] = [];
  const [status, setStatus] = createSignal<'online' | 'asleep'>('online');
  const client = {
    peers: { status: () => () => ({ kind: status() }) },
    commands: {
      run: async (_c: unknown, a: { peer: string; agent: string; cwd?: string }) => {
        asked.push(`${a.peer} ${a.agent} ${a.cwd ?? '-'}`);
        if (o.fail?.()) throw new Error('away');
        return a.agent === 'claude' ? 'Opus' : null;
      },
    },
  } as unknown as Client;
  return { client, asked, setStatus };
}

test('one_ask_per_computer_harness_and_folder_shared_by_every_reader', async () => {
  const { client, asked } = stub();
  let clock = 0;
  const a = defaultModels(client, () => clock);
  const b = defaultModels(client, () => clock);
  expect(b).toBe(a);
  createRoot(() => {
    expect(a.word('local', 'claude', '/code/app')).toBeUndefined();
  });
  await until(() => a.word('local', 'claude', '/code/app') === 'Opus');
  expect(b.word('local', 'claude', '/code/app')).toBe('Opus');
  a.word('local', 'claude', '/code/app');
  expect(asked).toEqual(['local claude /code/app']);
  // can't tell: undefined, asked once too
  a.word('local', 'codex', '/code/app');
  await until(() => asked.length === 2);
  expect(a.word('local', 'codex', '/code/app')).toBeUndefined();
  expect(asked).toHaveLength(2);
  // no folder, Auto or no harness: nothing to ask
  expect(a.word('local', 'claude', undefined)).toBeUndefined();
  expect(a.word('local', 'auto', '/code/app')).toBeUndefined();
  expect(asked).toHaveLength(2);
});

test('a_computer_that_isnt_online_is_not_asked_until_it_is', async () => {
  const { client, asked, setStatus } = stub();
  const m = defaultModels(client, () => 0);
  setStatus('asleep');
  expect(m.word('box', 'claude', '/srv/app')).toBeUndefined();
  expect(asked).toEqual([]);
  setStatus('online');
  m.word('box', 'claude', '/srv/app');
  await until(() => m.word('box', 'claude', '/srv/app') === 'Opus');
  expect(asked).toEqual(['box claude /srv/app']);
});

test('a_failed_ask_waits_a_minute_before_the_next', async () => {
  let failing = true;
  const { client, asked } = stub({ fail: () => failing });
  let clock = 1000;
  const m = defaultModels(client, () => clock);
  m.word('local', 'claude', '/code/app');
  await until(() => asked.length === 1);
  await new Promise((r) => setTimeout(r, 0));
  // every read inside the minute: no new ask
  for (let i = 0; i < 5; i++) m.word('local', 'claude', '/code/app');
  clock += 30_000;
  m.word('local', 'claude', '/code/app');
  expect(asked).toHaveLength(1);
  // past it: asked again, and this time it answers
  failing = false;
  clock += 31_000;
  m.word('local', 'claude', '/code/app');
  await until(() => m.word('local', 'claude', '/code/app') === 'Opus');
  expect(asked).toHaveLength(2);
});

// the one rule each place names a run's model by: the model it was given, else what its Default runs, else the words
test('a_runs_model_is_its_own_else_what_default_runs_else_the_words', () => {
  // its own as the run recorded it (Compare's words before), never replaced by what Default runs now
  expect(runModelWord('claude-opus-5-5', 'Sonnet', 'Default model')).toBe('claude-opus-5-5');
  expect(runModelWord('gpt-6.1-sol', undefined, 'Default model')).toBe('gpt-6.1-sol');
  expect(runModelWord('', 'Opus', 'Default model')).toBe('Opus');
  expect(runModelWord(undefined, undefined, 'Default model')).toBe('Default model');
});

// Audit C #17: a fresh Home sent runtime.defaultModel about 20 times before the first launch: every list row and sidebar
// card asked for its own folder, and each worktree task has one of its own (home/worktrees/proj-<id>). Only the folder in
// focus asks (Home's chip for its project, the open task's): once per harness picked. The rows read that answer when
// it's their folder, else the computer's harnesses topic.
test('the_lists_rows_never_ask_only_the_folder_in_focus_does_audit_c_17', async () => {
  const { client, asked } = stub();
  const rows = Array.from({ length: 20 }, (_, i) => `/home/.wisp/worktrees/proj-${i}`);
  createRoot(() => {
    for (const dir of rows) expect(defaultModelName(client as never, 'local', 'claude', dir)).toBeUndefined();
  });
  await new Promise((r) => setTimeout(r, 0));
  expect(asked, 'no row asked').toEqual([]);
  // Home's chip asks for its project, once per harness
  const m = defaultModels(client);
  m.word('local', 'claude', '/code/app');
  m.word('local', 'claude', '/code/app');
  await until(() => m.word('local', 'claude', '/code/app') === 'Opus');
  m.word('local', 'codex', '/code/app');
  await until(() => asked.length === 2);
  expect(asked).toEqual(['local claude /code/app', 'local codex /code/app']);
  // a row in that folder reads its answer, without asking again
  expect(defaultModelName(client as never, 'local', 'claude', '/code/app')).toBe('Opus');
  expect(asked).toHaveLength(2);
});
