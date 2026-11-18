// @vitest-environment happy-dom
// perf r1 #6: opening a task answers the click in its own frame with the page's head and the turns' skeleton; the body
// (turns, reply box, details) is built the frame after, and then the reply box takes the caret, unless the focus moved
// meanwhile (a click on the head's ⋯ keeps its menu open). Before, the whole body was built in the click: 399 ms at 4×.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
import { createShell, nullPlatform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.body.innerHTML = '';
});

const WRITES = /^runtime\.(fork|followUp|create|rename|rerun)$|^goal\.|^crew\.|^automations\.(save|create)/;

async function openTask(o: { readonly page?: boolean } = {}) {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def) shell.register(def);
  // every command the UI sends, by id
  const sent: { id: string; args: Record<string, unknown> }[] = [];
  const run = client.commands.run.bind(client.commands);
  client.commands.run = ((cmd: { id: string }, args: Record<string, unknown>) => (sent.push({ id: cmd.id, args }), run(cmd as never, args as never))) as typeof client.commands.run;
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  const sessions = (): readonly { id: string; title: string; state: string; agent: string }[] => client.entities.list<{ id: string; title: string; state: string; agent: string }>('session')();
  const task = (): { id: string; title: string } | undefined => sessions().find((r) => r.title.startsWith('Add rate limiting'));
  await until(() => task() !== undefined);
  if (o.page !== false) {
    client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
    await until(() => root.querySelector('[data-part~="split.more"]') !== null);
  }
  return { client, root, sent, sessions, task: task() as { id: string; title: string } };
}


const reply = (root: HTMLElement): HTMLTextAreaElement | null => root.querySelector('[data-part~="split.body"] textarea, [data-kit="reply-box"] textarea, textarea');

test('a_task_opens_with_its_head_and_skeleton_first_then_its_body_then_the_caret_in_the_reply_box_perf_r1_6', async () => {
  const { client, root, task } = await openTask({ page: false });
  // what the page shows after each change of the DOM (a microtask, before any frame): a slow machine can run the next
  // frame before a poll looks, so the order is recorded, not polled
  const seen: string[] = [];
  const look = (): void => {
    const head = root.querySelector('[data-part~="split.more"]') !== null;
    const skeleton = root.querySelector('[data-part~="split.placeholder"]') !== null;
    const state = !head ? 'none' : reply(root) !== null ? (skeleton ? 'both' : 'body') : skeleton ? 'head+skeleton' : 'head';
    if (seen.at(-1) !== state) seen.push(state);
  };
  const watch = new MutationObserver(look);
  watch.observe(root, { childList: true, subtree: true });
  client.nav.navigate({ t: 'openTask', id: task.id as EntityKey as never });
  look();
  // the navigation's own render: the head and the skeleton, no turns or reply box yet; the frame after: the body, the
  // skeleton gone
  await until(() => reply(root) !== null && root.querySelector('[data-part~="split.placeholder"]') === null);
  watch.disconnect();
  expect(seen.filter((x) => x !== 'none')).toEqual(['head+skeleton', 'body']);
  // and then the caret
  await until(() => document.activeElement === reply(root));
  expect(document.activeElement).toBe(reply(root));
});

test('a_click_on_the_head_before_the_body_came_keeps_its_focus_the_reply_box_doesnt_take_it', async () => {
  const { client, root, task } = await openTask({ page: false });
  client.nav.navigate({ t: 'openTask', id: task.id as EntityKey as never });
  await until(() => root.querySelector('[data-part~="split.more"]') !== null);
  const more = root.querySelector('[data-part~="split.more"]') as HTMLElement;
  more.click();
  more.focus();
  await until(() => reply(root) !== null);
  await new Promise((r) => setTimeout(r, 100)); // the frames after the body: the caret would have moved by now
  expect(document.activeElement).not.toBe(reply(root));
  // its menu stayed open (the caret moving in would have closed it)
  await until(() => document.querySelectorAll('[data-kit="menu-item"]').length > 0);
  expect(document.querySelectorAll('[data-kit="menu-item"]').length).toBeGreaterThan(0);
});


test('a_task_opened_from_an_overlay_still_gets_the_caret_when_the_overlay_puts_focus_back_as_it_closes', async () => {
  const { client, root, task } = await openTask({ page: false });
  // the palette's Open: its input has the focus, the task opens, the palette closes and gives the focus back to what
  // held it before (here a list row); no click on the page
  const before = document.createElement('button');
  const palette = document.createElement('input');
  document.body.append(before, palette);
  palette.focus();
  client.nav.navigate({ t: 'openTask', id: task.id as EntityKey as never });
  await until(() => root.querySelector('[data-part~="split.more"]') !== null);
  palette.remove();
  before.focus();
  await until(() => document.activeElement === reply(root));
  expect(document.activeElement).toBe(reply(root));
});

test('new_task_puts_the_caret_in_its_prompt_after_home_paints_not_in_the_click_perf_r1_10', async () => {
  const { client, root } = await openTask();
  (document.activeElement as HTMLElement | null)?.blur?.();
  client.nav.navigate({ t: 'home' });
  const prompt = (): HTMLTextAreaElement | null => root.querySelector('[data-part~="launcher.input"]');
  await until(() => prompt() !== null);
  // the navigation's own render: Home is there, the caret isn't moved yet (that focus forced a style and layout of it)
  expect(document.activeElement).not.toBe(prompt());
  await until(() => document.activeElement === prompt());
  expect(document.activeElement).toBe(prompt());
});
