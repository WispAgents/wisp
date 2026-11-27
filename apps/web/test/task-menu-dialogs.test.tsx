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

// goal.declared is a read (the reply box's Goal… menu loads it on open, ADR-0110): every other goal.* writes
const WRITES = /^runtime\.(fork|followUp|create|rename|rerun)$|^goal\.(?!declared$)|^crew\.|^automations\.(save|create)/;

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
  // the page's keys come with a lazy chunk (builtins-later.ts): press until it's in, as a user would press again
  const press = (): boolean => document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'F', code: 'KeyF', shiftKey: true, bubbles: true }));
  await until(() => (press(), dialog('fork.modal') !== null));
  expect(dialog('fork.modal'), 'Shift+F opens the fork dialog').not.toBeNull();
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

// QA round 15 #2: one Rename, one dialog (v1 actions.js rename: the title selected, Enter keeps, Escape leaves it,
// an empty or unchanged title sends nothing), from the head's ⋯ and a row's menu alike
async function renameDialog(): Promise<{ input: HTMLInputElement; save: HTMLButtonElement }> {
  await until(() => document.activeElement === dialog('list.rename')?.querySelector('input'));
  const d = dialog('list.rename') as HTMLElement;
  return { input: d.querySelector('input') as HTMLInputElement, save: d.querySelector('[data-part~="list.rename.save"]') as HTMLButtonElement };
}

test('rename_in_the_heads_menu_is_the_one_rename_dialog_qa_r14_3_r15_2', async () => {
  const { root, sent, task } = await openTask();
  await pick(root, /^Rename/);
  const { input, save } = await renameDialog();
  expect(input.value).toBe(task.title);
  expect([input.selectionStart, input.selectionEnd], 'the title selected, to type over').toEqual([0, task.title.length]);
  expect(save.disabled, 'nothing to save until it changes').toBe(true);
  expect(document.querySelectorAll('[data-part~="task.rename"], [data-part~="phone.task.rename"]').length, 'no second dialog').toBe(0);
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until(() => dialog('list.rename') === null);
  expect(writes(sent)).toEqual([]);
});

test('rename_in_a_rows_menu_is_the_same_dialog_qa_r15_2', async () => {
  const { root, sent, task } = await openTask({ page: false });
  const row = (): HTMLElement | undefined => [...root.querySelectorAll<HTMLElement>('[data-kit="list-row"]')].find((r) => r.textContent?.includes(task.title));
  await until(() => row() !== undefined);
  row()?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 40, clientY: 40 }));
  await until(() => item(/^Rename/) !== undefined);
  item(/^Rename/)?.click();
  const { input, save } = await renameDialog();
  expect(input.value).toBe(task.title);
  expect(save.disabled).toBe(true);
  input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until(() => dialog('list.rename') === null);
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
    // the dialog opened: Add to crew… did nothing when the crew plugin's later part landed while Task options was open
    // (a 4 s wait that passed anyway, then a timeout on Windows CI; the kit's menu now keys rows in place)
    expect(client.nav.nav().overlays.map((o) => o.id)).toContain(part);
    expect(writes(sent)).toEqual([]);
  });
}

test('fork_from_a_row_reads_the_tasks_turns_before_deciding_qa_r15_1', async () => {
  // the list's Fork… opens the dialog with no task page open: no turns of it in the store yet
  const { client, sent, sessions, task } = await openTask({ page: false });
  const before = sessions().length;
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.fork' as never, params: { id: task.id } } });
  await until(() => dialog('fork.modal')?.querySelector('textarea') != null);
  await until(() => /whole conversation|conversation up to/.test(dialog('fork.modal')?.textContent ?? ''));
  expect(dialog('fork.modal')?.textContent).not.toMatch(/Nothing to fork yet/);
  expect(writes(sent)).toEqual([]);
  expect(sessions().length).toBe(before);
});
