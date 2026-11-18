// @vitest-environment happy-dom
// Audit A (tasks), the list's and the task page's keys and their safety nets:
//   #4  Delete from the head's ⋯, ⌫ or Delete deleted at once: it asks first, as the row's Delete (v1 actions.js:345).
//   #11 After ⌫ deleted the picked row, Enter opened the deleted task ("no task <id>").
//   #12 L, M, ⇧R and V did nothing on the picked row (v1 keys.js acts on the focused row).
//   #13 E on the picked row and Archive from the task's ⋯ had no Undo (the row's buttons do).
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
  // a field a test focused (Preview's command) leaves with the page: nothing stays focused into the next test
  (document.activeElement as HTMLElement | null)?.blur?.();
  document.body.innerHTML = '';
});

type Row = { id: string; title: string; state: string; archived?: boolean; summary?: Record<string, unknown> };

async function start() {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const toaster = createToaster();
  // what each toast said, and its action (Undo)
  const toasts: { message: string; action?: { label: string; onClick: () => void } }[] = [];
  for (const k of ['show', 'success'] as const) {
    const was = toaster[k].bind(toaster);
    (toaster as unknown as Record<string, unknown>)[k] = (message: string, o?: { action?: { label: string; onClick: () => void } }) => (toasts.push({ message, ...(o?.action && { action: o.action }) }), was(message, o as never));
  }
  const shell = createShell({ client, toaster, platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def) shell.register(def);
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
  const rows = (): readonly Row[] => client.entities.list<Row>('session')();
  const keyOf = (title: RegExp): EntityKey => client.entities.keys('session')().find((k) => title.test(client.entities.peek<Row>('session', k)?.title ?? '')) as EntityKey;
  await until(() => rows().some((r) => r.title.startsWith('Add rate limiting')));
  // the list has drawn its rows: the keys act on the picked one
  await until(() => document.querySelector('[data-part="card"][data-id]') !== null);
  return { client, shell, root, sent, rows, keyOf, toasts };
}

const press = (key: string, o: KeyboardEventInit = {}): void => void document.body.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...o }));
const dialog = (part: string): HTMLElement | null => document.querySelector(`[data-part~="${part}"]`);
const sentIds = (sent: readonly { id: string }[]): string[] => sent.map((c) => c.id);

test('delete_on_the_open_task_asks_first_and_only_the_confirm_deletes_audit_a_4', async () => {
  const { client, sent, keyOf } = await start();
  const key = keyOf(/^Add rate limiting/);
  client.nav.navigate({ t: 'openTask', id: key as never });
  await until(() => document.querySelector('[data-part~="split.more"]') !== null);
  press('Backspace');
  await until(() => dialog('task.delete') !== null);
  expect(dialog('task.delete.modal')?.textContent).toContain('Delete this task from wisp?');
  expect(dialog('task.delete')?.textContent).toContain('The agent’s own history is kept.');
  expect(sentIds(sent), 'nothing deleted by the key itself').not.toContain('runtime.delete');
  // Cancel keeps it
  (dialog('task.delete.cancel') as HTMLElement).click();
  await until(() => dialog('task.delete') === null);
  expect(sentIds(sent)).not.toContain('runtime.delete');
  // Delete (the key again), then the confirm
  press('Delete');
  await until(() => dialog('task.delete.ok') !== null);
  (dialog('task.delete.ok') as HTMLElement).click();
  await until(() => sentIds(sent).includes('runtime.delete'));
  expect(sent.find((c) => c.id === 'runtime.delete')?.args['id']).toBe(key);
});

test('after_the_picked_row_is_deleted_enter_opens_nothing_audit_a_11', async () => {
  const { client, shell, sent, keyOf } = await start();
  const key = keyOf(/^Add rate limiting/);
  shell.context.selection.set(key);
  press('Backspace');
  await until(() => dialog('task.delete.ok') !== null);
  (dialog('task.delete.ok') as HTMLElement).click();
  await until(() => sentIds(sent).includes('runtime.delete'));
  expect(shell.context.selection.get(), 'the deleted row is no longer picked').not.toBe(key);
  press('Enter');
  await new Promise((r) => setTimeout(r, 50));
  const main = client.nav.nav().main as { kind: string; id?: unknown };
  expect(main.kind === 'task' && String(main.id) === key, 'never opens the deleted task').toBe(false);
});

test('the_picked_rows_keys_act_on_it_rename_rerun_and_save_for_later_audit_a_12', async () => {
  const { client, shell, sent, keyOf } = await start();
  const key = keyOf(/^Add rate limiting/);
  shell.context.selection.set(key);
  press('m');
  await until(() => client.nav.nav().overlays.some((o) => String(o.id) === 'shell.rename'));
  expect(client.nav.nav().overlays.find((o) => String(o.id) === 'shell.rename')?.params).toEqual({ id: key });
  client.nav.navigate({ t: 'closeOverlay' });
  press('R', { shiftKey: true });
  await until(() => client.nav.nav().overlays.some((o) => String(o.id) === 'shell.rerun'));
  client.nav.navigate({ t: 'closeOverlay' });
  press('l');
  await until(() => sentIds(sent).includes('runtime.forLater'));
  expect(sent.find((c) => c.id === 'runtime.forLater')?.args['id']).toBe(key);
  // V: the task opens with its steps (the page's own Transcript)
  press('v');
  await until(() => (client.nav.nav().main as { kind: string }).kind === 'task');
  expect(String((client.nav.nav().main as { id?: unknown }).id)).toBe(key);
});

test('marking_the_picked_row_done_and_archiving_from_the_tasks_menu_offer_undo_audit_a_13', async () => {
  const { client, shell, sent, keyOf, toasts } = await start();
  const toast = (text: RegExp) => toasts.find((x) => text.test(x.message));
  const key = keyOf(/^Why is the list query slow/);
  shell.context.selection.set(key);
  press('e');
  await until(() => toast(/^Marked done/) !== undefined);
  expect(toast(/^Marked done/)?.action?.label).toBe('Undo');
  toast(/^Marked done/)?.action?.onClick();
  await until(() => sent.filter((c) => c.id === 'runtime.setState').length === 2);
  expect(sent.filter((c) => c.id === 'runtime.setState').map((c) => c.args['state'])).toEqual(['done', 'review']);
  // the task's ⋯ → Archive (A on the open task: the same item)
  const other = keyOf(/^Review PR #806/);
  client.nav.navigate({ t: 'openTask', id: other as never });
  await until(() => document.querySelector('[data-part~="split.more"]') !== null);
  press('a');
  await until(() => toast(/^Archived/) !== undefined);
  toast(/^Archived/)?.action?.onClick();
  await until(() => sent.filter((c) => c.id === 'runtime.archive').length === 2);
  expect(sent.filter((c) => c.id === 'runtime.archive').map((c) => c.args['archived'])).toEqual([true, false]);
});

// Audit A #9: the head's "Changes +n −n" added up every turn's edits (+6 −6 after two follow-ups to a one-line fix, and
// still there after Revert or after the PR merged): it says what git says is changed now, as the Changes pane does, and
// nothing once git has nothing to show (or the worktree is gone)
test('the_heads_changes_figures_are_whats_changed_now_not_every_turns_sum_audit_a_9', async () => {
  const { client, keyOf } = await start();
  let now: { stats: { path: string; add: number; del: number }[] } | null = { stats: [{ path: 'math.js', add: 1, del: 1 }] };
  const run = client.commands.run.bind(client.commands);
  client.commands.run = ((cmd: { id: string }, args: unknown) =>
    cmd.id === 'git.changes' ? Promise.resolve(now && { base: 'HEAD', at: 'x', untracked: 0, body: { kind: 'inline', files: [] }, ...now }) : run(cmd as never, args as never)) as typeof client.commands.run;
  const key = keyOf(/^Add rate limiting/);
  client.nav.navigate({ t: 'openTask', id: key as never });
  const figures = (): string => document.querySelector('[data-part~="split.changes"]')?.textContent?.replace(/\s+/g, ' ').trim() ?? '';
  await until(() => /\+1 −1/.test(figures()));
  expect(figures()).toMatch(/\+1 −1/);
  // reverted: git shows nothing, the head shows no figures
  now = null;
  client.nav.navigate({ t: 'home' });
  client.nav.navigate({ t: 'openTask', id: key as never });
  await until(() => document.querySelector('[data-part~="split.more"]') !== null && !/\+\d/.test(figures()));
  expect(figures()).not.toMatch(/\+\d/);
});

// Audit A #17: Preview's Run did nothing with "Runs with" empty: it says what it needs and puts the caret there
test('previews_run_with_no_command_says_what_it_needs_audit_a_17', async () => {
  const { client, sent, keyOf } = await start();
  const key = keyOf(/^Add rate limiting/);
  client.nav.navigate({ t: 'openTask', id: key as never });
  await until(() => document.querySelector('[data-part~="split.more"]') !== null);
  client.nav.navigate({ t: 'toggleSide', side: { kind: 'preview', id: key as never } });
  const field = (): HTMLInputElement | null => document.querySelector('[data-kit="run-preview"] #rn-cmd');
  await until(() => field() !== null);
  const box = field() as HTMLInputElement;
  box.value = '';
  box.dispatchEvent(new InputEvent('input', { bubbles: true }));
  const runButton = [...document.querySelectorAll<HTMLButtonElement>('[data-kit="run-preview"] [data-kit="rn-row"] button')].at(-1);
  runButton?.click();
  await until(() => document.querySelector('[data-kit="run-preview"] [data-kit="rn-said"]') !== null);
  expect(document.querySelector('[data-kit="run-preview"] [data-kit="rn-said"]')?.textContent).toBe('Type the command that starts the app, then Run.');
  expect(document.activeElement).toBe(field());
  expect(sent.map((c) => c.id)).not.toContain('run.start');
});

// Audit A #19: the side panel's place ⋯ offered only "Copy branch name": it read the task's folder from the row's top
// (synced rows keep it under `body`), so Copy path, Open in editor and Open in terminal app never showed
test('the_side_panels_place_menu_offers_the_tasks_folder_audit_a_19', async () => {
  const { client, keyOf } = await start();
  const key = keyOf(/^Add rate limiting/);
  client.nav.navigate({ t: 'openTask', id: key as never });
  await until(() => document.querySelector('[data-part~="split.more"]') !== null);
  client.nav.navigate({ t: 'openSide', side: { kind: 'changes', id: key as never } });
  // the menu's button (its content carries `panel.place.more` too)
  const more = (): HTMLElement | null => document.querySelector('[data-part~="panel.place.more.trigger"]');
  await until(() => more() !== null);
  // the menu's machine first (a hover or focus warms it; the app preloads it after the first screen), then the press
  more()?.focus();
  await until(() => more()?.getAttribute('data-scope') === 'menu');
  more()?.click();
  const items = (): string[] => [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].map((e) => (e.textContent ?? '').trim());
  await until(() => items().some((x) => x.startsWith('Copy path')));
  expect(items().some((x) => x.startsWith('Copy path'))).toBe(true);
});
