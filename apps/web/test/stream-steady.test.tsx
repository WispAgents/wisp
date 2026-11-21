// @vitest-environment happy-dom
// perf r1 #12, #15: a task streaming restyles only what grows. Behind an open task on a phone, the Tasks bar's account
// menu was made again (a new menu machine) 18 times in 100 tokens, because each patch to any task made the bar new
// actions; on the desktop the open task's closed menus (More, the reply box's + and mode) rebuilt all their rows on
// each patch. Now a stream leaves them as they are.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

async function start(platform: Platform, width: number) {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def) shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, root, mock };
}

/** opens the mock's finished task, streams `tokens` into the running one, and counts what changed under `sel` */
async function streamUnder(o: { platform: Platform; width: number; ready: string; under: () => readonly Element[]; first?: () => Promise<void>; into?: 'open' }): Promise<{ changes: number; found: number; same: boolean }> {
  const { client, root, mock } = await start(o.platform, o.width);
  const task = (): { id: string } | undefined => client.entities.list<{ id: string; title: string }>('session')().find((r) => r.title.startsWith('Add rate limiting'));
  await until(() => task() !== undefined);
  client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
  await until(() => root.querySelector(o.ready) !== null);
  await new Promise((r) => setTimeout(r, 300)); // the body and its menus are drawn
  await o.first?.();
  const before = o.under();
  let changes = 0;
  const mo = new MutationObserver((l) => (changes += l.filter((m) => m.type === 'childList').length));
  for (const el of before) mo.observe(el, { subtree: true, childList: true });
  mock.stream(o.into === 'open' ? task()?.id : undefined, { tokens: 30, everyMs: 5 });
  await new Promise((r) => setTimeout(r, 600));
  mo.disconnect();
  const after = o.under();
  return { changes, found: before.length, same: before.length === after.length && before.every((x, i) => x === after[i] && x.isConnected) };
}

test('a_stream_behind_an_open_task_leaves_the_phone_tasks_bars_account_menu_as_it_is', async () => {
  const r = await streamUnder({
    platform: nullPlatform({ kind: 'android', phone: true, mobile: true }),
    width: 390,
    ready: '[data-part~="phone.task.bar"]',
    under: () => [...document.querySelectorAll('[data-part~="phone.tasks.bar.actions"], [data-part~="phone.tasks.me.trigger"]')],
  });
  expect(r.found, 'the bar and its account menu trigger (each platform variant)').toBeGreaterThanOrEqual(2);
  expect(r.same, 'the same bar and the same account menu trigger').toBe(true);
  expect(r.changes).toBe(0);
});

test('a_patch_like_a_streamed_tokens_leaves_the_open_tasks_menus_rows_as_they_are_on_the_desktop', async () => {
  const { client, root, mock } = await start(nullPlatform({ kind: 'browser' }), 1440);
  const task = (): { id: string } | undefined => client.entities.list<{ id: string; title: string }>('session')().find((r) => r.title.startsWith('Add rate limiting'));
  await until(() => task() !== undefined);
  const id = task()?.id ?? '';
  client.nav.navigate({ t: 'openTask', id: id as EntityKey as never });
  await until(() => root.querySelector('[data-part~="split.more"]') !== null);
  const PARTS = ['split.more', 'composer.plus', 'composer.mode'];
  const menuOf = (part: string): Element | undefined => {
    const trig = document.querySelector(`[data-part~="${part}"]`);
    return [...document.querySelectorAll('[role="menu"]')].find((m) => trig?.id && m.getAttribute('aria-labelledby') === trig.id);
  };
  // what a streamed token does to the task's row now and then (the mock's every 20th): its live line and its time
  const topic = `session:${id.slice(id.indexOf(':') + 1)}`;
  mock.handle('test.token', () => ({
    ops: ['list', topic].map((t) => ({ topic: t as never, e: 'session', id, op: 'patch' as const, ops: [{ op: 'replace' as const, path: '/summary/live', value: `Writing… ${Math.random()}` }, { op: 'replace' as const, path: '/updated_at', value: Date.now() }] })),
  }));
  const token = { id: 'test.token', owner: () => 'local', optimistic: 'none' as const, pending: 'a token' };
  // each menu open while the tokens land: its rows stay the same nodes (they were thrown away and drawn again)
  for (const part of PARTS) {
    await until(() => document.querySelector(`[data-part~="${part}"]`) !== null);
    (document.querySelector(`[data-part~="${part}"]`) as HTMLElement).click();
    await until(() => menuOf(part)?.querySelector('[data-kit="menu-item"]') != null);
    const rows = (): Element[] => [...(menuOf(part)?.querySelectorAll('[data-kit="menu-item"]') ?? [])];
    const before = rows();
    expect(before.length, `${part} has rows`).toBeGreaterThan(1);
    for (let k = 0; k < 3; k++) await client.commands.run(token as never, {} as never);
    await new Promise((r) => setTimeout(r, 150));
    const after = rows();
    expect(after.length, part).toBe(before.length);
    expect(after.every((x, k) => x === before[k] && x.isConnected), `${part}: the same row nodes`).toBe(true);
    menuOf(part)?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until(() => menuOf(part)?.getAttribute('data-state') !== 'open');
  }
});
