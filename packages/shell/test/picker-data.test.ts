// perf-ui-r2 #3: every open of the harness picker sent 10 requests (runtime.agents, accounts.list, the harnesses topic
// and one budget.view per harness, 7) and applied the usage answers one by one. Kept per peer: a hover warms it, an
// open soon after asks nothing, and the usage lands as one map.
import { expect, test } from 'vitest';
import { FRESH_MS, OPEN_MS, holdHarnesses, pickerData } from '../src/frame/picker-data.ts';

const HARNESSES = ['claude', 'codex', 'opencode', 'cursor', 'copilot', 'grok', 'antigravity'];

function server() {
  const asked: string[] = [];
  const run = (cmd: { id: string }, args: unknown): Promise<never> => {
    asked.push(cmd.id);
    const a = args as { agent?: string };
    const out =
      cmd.id === 'runtime.agents'
        ? HARNESSES.map((id) => ({ id, name: id, available: id !== 'grok', models: [] }))
        : cmd.id === 'budget.view'
          ? [{ windows: [{ label: '5h', used: a.agent === 'claude' ? 0.42 : 0.1 }] }]
          : { accounts: [{ id: 'a1', label: 'Work' }] };
    return Promise.resolve(out as never);
  };
  return { run: run as never, asked };
}

const count = (asked: readonly string[], id: string): number => asked.filter((x) => x === id).length;

test('a_hover_warms_the_picker_and_an_open_soon_after_sends_no_request', async () => {
  const s = server();
  let now = 0;
  const d = pickerData(s.run, 'laptop', () => now);
  d.warm('claude');
  await new Promise((r) => setTimeout(r, 0));
  expect(count(s.asked, 'runtime.agents')).toBe(1);
  expect(count(s.asked, 'budget.view'), 'each set-up harness once').toBe(6);
  expect(count(s.asked, 'accounts.list')).toBe(1);
  const before = s.asked.length;

  // the open: everything is here at once, nothing asked
  now += 1000;
  expect(d.agentsNow()?.length).toBe(7);
  const ids = (d.agentsNow() ?? []).filter((a) => a.available).map((a) => a.id);
  const usage = d.usageNow(ids);
  expect(usage.get('claude')?.used).toBe(0.42);
  expect(usage.size).toBe(6);
  await d.usage(ids);
  await d.accounts('claude');
  expect(s.asked.length, 'a second open within FRESH_MS asks nothing').toBe(before);

  // past FRESH_MS it asks again, once per answer
  now += FRESH_MS;
  await d.agents();
  await d.usage(ids);
  expect(count(s.asked, 'runtime.agents')).toBe(2);
  expect(count(s.asked, 'budget.view')).toBe(12);
});

test('two_opens_while_an_answer_is_in_flight_share_it', async () => {
  const s = server();
  const d = pickerData(s.run, 'laptop', () => 0);
  await Promise.all([d.usage(['claude', 'codex']), d.usage(['claude', 'codex']), d.agents(), d.agents()]);
  expect(count(s.asked, 'budget.view')).toBe(2);
  expect(count(s.asked, 'runtime.agents')).toBe(1);
});

test('the_harnesses_topic_stays_watched_a_while_after_the_picker_closes_then_stops', () => {
  const watched: string[] = [];
  const timers: (() => void)[] = [];
  let disposed = 0;
  const scope = {
    child: () => ({ ...scope, dispose: () => disposed++ }),
    timeout: (_ms: number, fn: () => void) => (timers.push(fn), () => timers.splice(timers.indexOf(fn), 1)),
    dispose: () => {},
  };
  const client = { peers: { watch: (_p: string, t: string) => watched.push(t) }, scope } as never;
  const release = holdHarnesses(client, 'laptop', 'plugin:wisp.runtime:harnesses' as never);
  release();
  expect(disposed, 'kept after closing').toBe(0);
  // reopened before the linger ends: the same watch, the linger cancelled
  const again = holdHarnesses(client, 'laptop', 'plugin:wisp.runtime:harnesses' as never);
  expect(watched).toHaveLength(1);
  expect(timers).toHaveLength(0);
  again();
  expect(timers).toHaveLength(1);
  timers[0]?.();
  expect(disposed).toBe(1);
  holdHarnesses(client, 'laptop', 'plugin:wisp.runtime:harnesses' as never);
  expect(watched, 'a later open subscribes again').toHaveLength(2);
});

test('a_failed_answer_is_not_kept_and_the_next_open_asks_again', async () => {
  let fail = true;
  let asked = 0;
  const run = ((cmd: { id: string }) => {
    asked++;
    if (fail) return Promise.reject(new Error('link dropped'));
    return Promise.resolve((cmd.id === 'runtime.agents' ? [{ id: 'claude', available: true }] : { accounts: [] }) as never);
  }) as never;
  const d = pickerData(run, 'laptop', () => 0);
  expect(await d.agents(), 'refused: nothing to list now').toEqual([]);
  expect(d.agentsNow()).toBeUndefined();
  fail = false;
  expect((await d.agents()).map((a) => a.id), 'asked again, not 30 s of an empty list').toEqual(['claude']);
  expect(asked).toBe(2);
});

test('each_open_asks_the_usage_again_unless_a_hover_just_did_and_no_window_reads_null', async () => {
  const s = server();
  let now = 0;
  const d = pickerData(s.run, 'laptop', () => now);
  await d.usage(['claude'], OPEN_MS);
  expect(count(s.asked, 'budget.view')).toBe(1);
  now += OPEN_MS - 1;
  await d.usage(['claude'], OPEN_MS);
  expect(count(s.asked, 'budget.view'), 'a hover just read it').toBe(1);
  now += 2;
  await d.usage(['claude'], OPEN_MS);
  expect(count(s.asked, 'budget.view'), 'the next open reads it again, well inside FRESH_MS').toBe(2);
  const none = pickerData(((cmd: { id: string }) => Promise.resolve((cmd.id === 'budget.view' ? [{ windows: [] }] : []) as never)) as never, 'laptop', () => 0);
  expect((await none.usage(['cursor'])).get('cursor'), 'read, no window: null (limits not reported)').toBeNull();
});
