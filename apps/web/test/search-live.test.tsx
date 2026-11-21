// @vitest-environment happy-dom
// Search over data that arrives or changes while it is open (round 323): the palette (⌘K, the phone apps' search view,
// ⌘P) lists the tasks in the entity store as they are now, not a copy taken when it opened. Opened before the list's
// snapshot landed, its task shows when the snapshot does; a task renamed to match joins, one deleted leaves; the rows
// that didn't change keep their nodes (nothing jumps, no entry animation replays). v1 copied paletteItems() once at
// open (web/js/features/commands.js:236): docs/v1-bugs-fixed.md.
import { afterEach, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { createClient, peerId, type Frame, type Transport } from '@wisp/client';
import type { ClientMessage } from '@wisp/protocol';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer, type MockPeer } from '../dev/mock-peer.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

/** the mock peer in memory, its connection held until `open()`: the app runs, its store still empty */
function heldTransport(peer: MockPeer): { transport: Transport; open: () => void } {
  let open!: () => void;
  const gate = new Promise<void>((r) => (open = r));
  let clientId = '';
  const transport: Transport = {
    kind: 'mem',
    async *connect(params, signal): AsyncGenerator<Frame> {
      await gate;
      const frames: Frame[] = [];
      let wake: (() => void) | undefined;
      let closed = false;
      const conn = peer.connect(params, (f) => {
        frames.push(f);
        wake?.();
      });
      clientId = conn.clientId;
      signal.addEventListener('abort', () => {
        closed = true;
        conn.close();
        wake?.();
      });
      while (!closed) {
        const f = frames.shift();
        if (f) yield f;
        else await new Promise<void>((r) => (wake = r));
      }
    },
    async post(m: ClientMessage) {
      peer.receive(clientId, m);
    },
  };
  return { transport, open };
}

async function start(platform: Platform) {
  const mock = createMockPeer({ epoch: 'e1' });
  const held = heldTransport(mock);
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'mock', transport: held.transport }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  // another client of the same peer: what it changes reaches this one as the server's patch, not an optimistic one
  mock.connect({ client_id: 'other' }, () => {});
  let n = 0;
  const other = (cmd: string, args: unknown): void => mock.receive('other', { t: 'mutate', mid: `other-${(n += 1)}`, cmd, args });
  return { client, shell, root, mock, open: held.open, other };
}

const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-kit="pal"] [data-kit="pal-row"]')];
const labels = (): string[] => rows().map((r) => r.querySelector('[data-kit="pal-label"]')?.textContent ?? '');
const rowOf = (title: string): HTMLElement | undefined => rows().find((r) => r.querySelector('[data-kit="pal-label"]')?.textContent?.startsWith(title));
/** types into the palette's box as a person does */
const type = (text: string): void => {
  const input = q('[data-kit="pal"] input') as HTMLInputElement;
  input.value = text;
  input.dispatchEvent(new Event('input', { bubbles: true }));
};
const bareId = (mock: MockPeer, title: string): string => {
  const id = mock.rows('list').find((r) => r.e === 'session' && (r.v as { title?: string }).title?.startsWith(title))?.id ?? '';
  return id.slice(id.indexOf(':') + 1);
};

const phone = (kind: 'android' | 'ios'): Platform => nullPlatform({ kind, phone: true, mobile: true });

for (const [name, platform] of [
  ['android', phone('android')],
  ['ios', phone('ios')],
  ['desktop', nullPlatform({})],
] as const) {
  test(`search_opened_before_the_list_arrives_shows_its_tasks_when_they_land_on_${name}`, async () => {
    const { client, open } = await start(platform);
    client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: null } });
    await until(() => q('[data-kit="pal"] input') !== null);
    expect(client.entities.list('session')(), 'the store is empty when search opens').toHaveLength(0);
    expect(labels().some((l) => l.startsWith('Add rate limiting'))).toBe(false);
    // the welcome and the list's snapshot land: the task shows, nothing retyped
    open();
    await until(() => labels().some((l) => l.startsWith('Add rate limiting')));
    expect(labels().some((l) => l.startsWith('Add rate limiting')), 'the late task in the results').toBe(true);
  });
}

test('a_query_typed_before_the_list_arrives_finds_the_task_when_it_lands_without_retyping', async () => {
  const { client, open } = await start(phone('android'));
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: null } });
  await until(() => q('[data-kit="pal"] input') !== null);
  type('rate limit');
  expect(rowOf('Add rate limiting')).toBeUndefined();
  open();
  await until(() => rowOf('Add rate limiting') !== undefined);
  expect(rowOf('Add rate limiting'), 'the query runs again on the snapshot').toBeDefined();
});

test('a_task_renamed_to_match_joins_the_results_one_deleted_leaves_and_the_others_keep_their_nodes', async () => {
  const { client, mock, open, other } = await start(nullPlatform({}));
  open();
  await until(() => client.entities.list<{ title: string }>('session')().some((r) => r.title.startsWith('Add rate limiting')));
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: null } });
  await until(() => q('[data-kit="pal"] input') !== null);
  // "sync": two tasks match ("Fix the flaky reconnect test in sync", "Review the reconnect fix against the sync spec")
  type('sync');
  await until(() => rowOf('Fix the flaky reconnect test') !== undefined);
  const kept = rowOf('Fix the flaky reconnect test');
  const gone = rowOf('Review the reconnect fix');
  expect(kept, 'a matching task').toBeDefined();
  expect(gone, 'a matching task to delete').toBeDefined();
  expect(rowOf('Add rate limiting'), 'no "sync" in it yet').toBeUndefined();

  // renamed elsewhere to match: it joins, the row that didn't change is the same node
  other('runtime.rename', { id: bareId(mock, 'Add rate limiting'), title: 'Add rate limiting to sync' });
  await until(() => rowOf('Add rate limiting to sync') !== undefined);
  expect(rowOf('Add rate limiting to sync'), 'the renamed task joins').toBeDefined();
  expect(rowOf('Fix the flaky reconnect test'), 'the unchanged row keeps its node').toBe(kept);

  // deleted elsewhere: it leaves, the others stay where they are
  other('runtime.delete', { id: bareId(mock, 'Review the reconnect fix') });
  await until(() => rowOf('Review the reconnect fix') === undefined);
  expect(rowOf('Review the reconnect fix'), 'the deleted task leaves').toBeUndefined();
  expect(rowOf('Fix the flaky reconnect test'), 'the unchanged row still keeps its node').toBe(kept);
  expect(gone?.isConnected).toBe(false);
});

// Open file… (shell-task) and New task's place list (shell) keep their source's state in a Solid signal: the list
// fills in when its command answers, so the palette follows those as well as the store, on the same pass
test('a_palette_source_that_reads_its_own_solid_state_fills_in_when_that_state_comes', async () => {
  const { client, open, shell } = await start(nullPlatform({}));
  const [files, setFiles] = createSignal<readonly string[]>([]);
  shell.register({ id: 'wisp.demo', palette: [{ id: 'demo.files', items: () => files().map((f) => ({ id: `file:${f}`, group: 'Files', label: f })) }] });
  open();
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: { only: ['demo.files'] } } });
  await until(() => q('[data-kit="pal"] input') !== null);
  expect(labels(), 'opened at once, empty').toEqual([]);
  setFiles(['src/login.ts', 'src/rate-limit.ts']);
  await until(() => labels().includes('rate-limit.ts') || labels().includes('src/rate-limit.ts'));
  expect(labels(), 'the source’s list once it came').toEqual(['src/login.ts', 'src/rate-limit.ts']);
});
