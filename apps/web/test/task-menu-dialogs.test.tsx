// @vitest-environment happy-dom
// QA round 14 desktop: a task menu's "…" items open their dialog and nothing happens until its confirm.
//   #1 Fork… forked at once (no dialog): it's v1's Fork task dialog (actions.js forkTask: what the fork should try, its
//      mode and harness, the turn it forks from), from the head's ⋯ and ⇧F alike; opening and cancelling it makes
//      nothing (QA must never create data by opening a menu).
//   #2 Task options › Set goal…, Add to crew…, Save as recipe… each open their dialog.
//   #3 Rename in the head's ⋯ opens the rename dialog, as the row's does.
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

async function openTask() {
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
  client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
  await until(() => root.querySelector('[data-part~="split.more"]') !== null);
  return { client, root, sent, sessions, task: task() as { id: string; title: string } };
}

const item = (label: RegExp): HTMLElement | undefined => [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].find((e) => label.test(e.textContent ?? ''));
async function pick(root: HTMLElement, ...path: RegExp[]): Promise<void> {
  (root.querySelector('[data-part~="split.more"]') as HTMLElement).click();
  for (const p of path) {
    await until(() => item(p) !== undefined);
    item(p)?.click();
  }
}
const writes = (sent: readonly { id: string }[]): string[] => sent.map((c) => c.id).filter((id) => WRITES.test(id));
const dialog = (part: string): HTMLElement | null => document.querySelector(`[data-part~="${part}"]`);

test('fork_from_the_heads_menu_opens_v1s_fork_dialog_and_cancelling_it_makes_nothing_qa_r14_1', async () => {
  const { root, sent, sessions } = await openTask();
  const before = sessions().length;
  await pick(root, /^Fork…/);
  await until(() => dialog('fork.modal')?.querySelector('textarea') != null);
  const modal = dialog('fork.modal') as HTMLElement;
  expect(modal.querySelector('textarea')?.getAttribute('placeholder')).toBe('What should the fork try?');
  // nothing to fork with until it's told what to try
  expect((modal.querySelector('[data-part~="fork.go"]') as HTMLButtonElement).disabled).toBe(true);
  expect(modal.textContent).toMatch(/on its own branch/);
  // Cancel: no fork, no new task
  (modal.querySelector('[data-part~="fork.cancel"]') as HTMLElement).click();
  await until(() => dialog('fork.modal') === null);
  expect(writes(sent), 'opening and cancelling sent nothing').toEqual([]);
  expect(sessions().length).toBe(before);
});

test('shift_f_opens_the_fork_dialog_too_and_makes_nothing_qa_r14_1', async () => {
  const { sent, sessions } = await openTask();
  const before = sessions().length;
  document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'F', code: 'KeyF', shiftKey: true, bubbles: true }));
  await until(() => dialog('fork.modal') !== null);
  expect(writes(sent)).toEqual([]);
  expect(sessions().length).toBe(before);
});

test('the_fork_starts_only_on_its_confirm_with_what_it_should_try_qa_r14_1', async () => {
  const { root, sent, task } = await openTask();
  await pick(root, /^Fork…/);
  await until(() => dialog('fork.modal')?.querySelector('textarea') != null);
  const ta = dialog('fork.modal')?.querySelector('textarea') as HTMLTextAreaElement;
  ta.value = 'Try it with a token bucket';
  ta.dispatchEvent(new InputEvent('input', { bubbles: true }));
  await until(() => !(dialog('fork.modal')?.querySelector('[data-part~="fork.go"]') as HTMLButtonElement | null)?.disabled);
  (dialog('fork.modal')?.querySelector('[data-part~="fork.go"]') as HTMLElement).click();
  await until(() => sent.some((c) => c.id === 'runtime.followUp'));
  const fork = sent.find((c) => c.id === 'runtime.fork');
  const follow = sent.find((c) => c.id === 'runtime.followUp');
  expect(fork?.args['id']).toBe(task.id);
  expect(follow?.args['id']).toBe(fork?.args['newId']);
  expect(follow?.args['text']).toBe('Try it with a token bucket');
  expect(sent.findIndex((c) => c.id === 'runtime.fork')).toBeLessThan(sent.findIndex((c) => c.id === 'runtime.followUp'));
});

test('rename_in_the_heads_menu_opens_the_rename_dialog_with_the_title_qa_r14_3', async () => {
  const { root, sent, task } = await openTask();
  await pick(root, /^Rename/);
  await until(() => dialog('task.rename') !== null);
  expect((dialog('task.rename')?.querySelector('input') as HTMLInputElement).value).toBe(task.title);
  expect(writes(sent)).toEqual([]);
});

for (const [label, part] of [
  [/^Set goal…/, 'goal.edit'],
  [/^Add to crew…/, 'crew.editor'],
  [/^Save as recipe…/, 'automations.recipe'],
] as const) {
  test(`task_options_${part}_opens_its_dialog_qa_r14_2`, async () => {
    const { root, client, sent } = await openTask();
    await pick(root, /^Task options/, label);
    await until(() => client.nav.nav().overlays.some((o) => o.id === part));
    expect(writes(sent)).toEqual([]);
  });
}
