// @vitest-environment happy-dom
// Audit ui-r1 doc24a/doc24b: a time picked on New task (Start later, or the hold row's Wait until the reset) or on a
// task's reply box (Send later) only reached the later plugin's plan when no file was attached (and, on a reply, no
// review comment waited): with them the task started, or the follow-up went, at once, the time ignored. A plan carries
// no files yet, so such a send is refused with a toast and everything left as it was; without a time, files go as
// before.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
import { createShell, nullPlatform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';
import { hop, until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  (document.activeElement as HTMLElement | null)?.blur?.();
  document.body.innerHTML = '';
});

type Extra = { readonly text: string; taken(): void; back(): void };

async function start(o: { readonly hold?: boolean; readonly extra?: { taken: number; back: number } } = {}) {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const toaster = createToaster();
  const toasts: string[] = [];
  for (const k of ['show', 'error'] as const) {
    const was = toaster[k].bind(toaster);
    (toaster as unknown as Record<string, unknown>)[k] = (message: string, opts?: unknown) => (toasts.push(message), was(message, opts as never));
  }
  const shell = createShell({ client, toaster, platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def) shell.register(def);
  // near the limit, with a reset to wait for: the hold row offers Wait until the reset
  if (o.hold) shell.register({ id: 'wisp.test-hold', sendHolds: [{ reason: 'test_hold', check: () => Promise.resolve({ reason: 'test_hold', say: 'Near the limit', until: Date.now() + 3_600_000 }) }] });
  // a review comment waiting to go with the next follow-up (wisp.comments' tray)
  const extra = o.extra;
  if (extra) {
    shell.register({
      id: 'wisp.test-comments',
      prompt: [{ id: 'test.comments', trigger: '/', where: ['reply'], items: () => [], pending: (text: string): Extra => ({ text: `> a comment\n\n${text}`, taken: () => void extra.taken++, back: () => void extra.back++ }) }],
    } as never);
  }
  const sent: { id: string; args: Record<string, unknown> }[] = [];
  const run = client.commands.run.bind(client.commands);
  client.commands.run = ((cmd: { id: string }, args: Record<string, unknown>) => (sent.push({ id: cmd.id, args }), run(cmd as never, args as never))) as typeof client.commands.run;
  // uploads answer at once (the mock peer has no /uploads route)
  const plug = client.plugin.bind(client);
  client.plugin = ((id: string) => {
    const pc = plug(id);
    if (id !== 'shell') return pc;
    return Object.assign(Object.create(pc) as object, {
      http: () => ({ send: (r: { path: string }) => Promise.resolve({ id: 'u1', name: decodeURIComponent(/name=([^&]*)/.exec(r.path)?.[1] ?? ''), mime: 'text/plain', size: 1 }) }),
    });
  }) as typeof client.plugin;
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  await until(() => client.entities.keys('session')().length > 0);
  return { client, root, sent, toasts };
}

const item = (re: RegExp): HTMLElement | undefined => [...document.querySelectorAll<HTMLElement>('[role^="menuitem"]')].find((e) => re.test((e.textContent ?? '').trim()));
const type = (box: HTMLTextAreaElement, v: string): void => {
  box.value = v;
  box.dispatchEvent(new InputEvent('input', { bubbles: true }));
};
/** a file dropped on the box */
const drop = (box: HTMLElement, f: File): void => {
  const e = new Event('drop', { bubbles: true, cancelable: true });
  Object.defineProperty(e, 'dataTransfer', { value: { files: [f], types: ['Files'], items: [] } });
  box.dispatchEvent(e);
};
/** + › Start later › In 1 hour */
async function later(root: HTMLElement, plus: string): Promise<void> {
  root.querySelector<HTMLElement>(`[data-part="${plus}"]`)?.click();
  await until(() => item(/^Start later/) !== undefined);
  item(/^Start later/)?.click();
  await until(() => item(/^In 1 hour/) !== undefined);
  item(/^In 1 hour/)?.click();
}
const ran = (sent: readonly { id: string }[], re: RegExp): number => sent.filter((c) => re.test(c.id)).length;

async function newTask(root: HTMLElement) {
  const input = (): HTMLTextAreaElement | null => root.querySelector('[data-part~="launcher.input"]');
  await until(() => input() !== null && root.querySelector('[data-part="launcher.plus"]') !== null);
  const box = input() as HTMLTextAreaElement;
  type(box, 'Fix the flaky test');
  drop(box, new File(['x'], 'notes.txt', { type: 'text/plain' }));
  const send = (): HTMLElement | null => root.querySelector('[data-part~="launcher.send"]');
  return { box, send };
}

test('new_task_start_later_with_files_is_refused_never_started_now_doc24a', async () => {
  const { root, sent, toasts } = await start();
  const { box, send } = await newTask(root);
  await later(root, 'launcher.plus');
  await until(() => /^Start at /.test(send()?.getAttribute('aria-label') ?? ''));
  send()?.click();
  await until(() => toasts.some((m) => m.startsWith("Files can't wait")));
  await hop();
  expect(toasts).toContain("Files can't wait for a later start yet: remove them, or start now.");
  expect(ran(sent, /^runtime\.(create|trial)$/), 'nothing started now').toBe(0);
  expect(ran(sent, /^later\./), 'and no plan without its files').toBe(0);
  // New task as it was: the words, the file, the time
  expect(box.value).toBe('Fix the flaky test');
  expect(root.textContent).toContain('notes.txt');
  expect(send()?.getAttribute('aria-label') ?? '').toMatch(/^Start at /);
});

test('new_task_wait_until_the_reset_with_files_is_refused_never_started_now_doc24a', async () => {
  const { root, sent, toasts } = await start({ hold: true });
  const { box, send } = await newTask(root);
  send()?.click();
  const wait = (): HTMLElement | null => root.querySelector('[data-kit="hold"] [data-part$="wait"]');
  await until(() => wait() !== null);
  wait()?.click();
  await until(() => toasts.some((m) => m.startsWith("Files can't wait")));
  await hop();
  expect(ran(sent, /^runtime\.(create|trial)$/), 'nothing started now').toBe(0);
  expect(box.value).toBe('Fix the flaky test');
  expect(root.textContent).toContain('notes.txt');
});

test('new_task_with_files_and_no_time_starts_once_with_them_doc24a', async () => {
  const { root, sent } = await start();
  const { send } = await newTask(root);
  await until(() => root.textContent?.includes('notes.txt') === true);
  send()?.click();
  await until(() => ran(sent, /^runtime\.create$/) > 0);
  await hop();
  expect(ran(sent, /^runtime\.create$/)).toBe(1);
  expect(sent.find((c) => c.id === 'runtime.create')?.args['attachments']).toEqual([{ id: 'u1', name: 'notes.txt', mime: 'text/plain', size: 1 }]);
});

async function reply(o: Parameters<typeof start>[0] = {}) {
  const s = await start(o);
  const review = (): { id: string } | undefined => s.client.entities.list<{ id: string; state: string }>('session')().find((r) => r.state === 'review');
  await until(() => review() !== undefined);
  const task = review();
  s.client.nav.navigate({ t: 'openTask', id: task?.id as EntityKey as never });
  const send = (): HTMLElement | null => s.root.querySelector('[data-part="composer.send"]');
  await until(() => send() !== null && s.root.querySelector('[data-part="composer.plus"]') !== null);
  const box = s.root.querySelector<HTMLTextAreaElement>('[data-part="composer.input"]') as HTMLTextAreaElement;
  type(box, 'and the docs');
  return { ...s, box, send };
}

test('a_reply_sent_later_with_files_is_refused_never_sent_now_doc24b', async () => {
  const { root, sent, toasts, box, send } = await reply();
  drop(box, new File(['x'], 'notes.txt', { type: 'text/plain' }));
  await later(root, 'composer.plus');
  await until(() => /^Send at /.test(send()?.getAttribute('aria-label') ?? ''));
  send()?.click();
  await until(() => toasts.some((m) => m.startsWith('Files and review comments')));
  await hop();
  expect(ran(sent, /^runtime\.followUp$/), 'no follow-up now').toBe(0);
  expect(box.value).toBe('and the docs');
  expect(root.textContent).toContain('notes.txt');
  expect(send()?.getAttribute('aria-label') ?? '').toMatch(/^Send at /);
});

test('a_reply_sent_later_with_review_comments_waiting_is_refused_and_keeps_them_doc24b', async () => {
  const extra = { taken: 0, back: 0 };
  const { root, sent, toasts, box, send } = await reply({ extra });
  await later(root, 'composer.plus');
  await until(() => /^Send at /.test(send()?.getAttribute('aria-label') ?? ''));
  send()?.click();
  await until(() => toasts.some((m) => m.startsWith('Files and review comments')));
  await hop();
  expect(ran(sent, /^runtime\.followUp$/), 'no follow-up now').toBe(0);
  expect(extra.taken, 'the comments stay in the tray').toBe(0);
  expect(box.value).toBe('and the docs');
});

test('a_reply_with_files_and_no_time_goes_once_with_them_doc24b', async () => {
  const { sent, box, send, root } = await reply();
  drop(box, new File(['x'], 'notes.txt', { type: 'text/plain' }));
  await until(() => root.textContent?.includes('notes.txt') === true);
  send()?.click();
  await until(() => ran(sent, /^runtime\.followUp$/) > 0);
  await hop();
  expect(ran(sent, /^runtime\.followUp$/)).toBe(1);
  expect(sent.find((c) => c.id === 'runtime.followUp')?.args['attachments']).toEqual([{ id: 'u1', name: 'notes.txt', mime: 'text/plain', size: 1 }]);
});
