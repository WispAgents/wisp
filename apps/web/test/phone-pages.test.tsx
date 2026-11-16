// @vitest-environment happy-dom
// ADR-0065 step 13 on phones (design/mobile.md §A.2): the task page, New task and Search in the phone apps, through the
// real client and shell against the mock peer. Each screen: the platform's variant (Android, iOS), v1's features on it,
// and a phone's browser (no data-platform) keeping v1's look.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster, loadPlatformStyles } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

async function start(platform: Platform) {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, shell, root };
}

const phone = (kind: 'android' | 'ios'): Platform => nullPlatform({ kind, phone: true, mobile: true });
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);

/** the mock's finished task, opened; back once every `ready` part is drawn (what the test then reads) */
async function openTask(client: Awaited<ReturnType<typeof start>>['client'], root: HTMLElement, ...ready: string[]): Promise<void> {
  const task = (): { id: string } | undefined => client.entities.list<{ id: string; title: string }>('session')().find((r) => r.title.startsWith('Add rate limiting'));
  await until(() => task() !== undefined);
  const first = task();
  expect(first, 'the mock’s finished task').toBeDefined();
  client.nav.navigate({ t: 'openTask', id: first?.id as EntityKey as never });
  await until(() => ready.every((sel) => root.querySelector(sel) !== null));
}
/** the phone's task page bar, its menu drawn */
const PHONE_TASK = '[data-part="phone.task.bar"] [data-kit="app-bar-actions"] > [aria-label="More"]';

for (const kind of ['android', 'ios'] as const) {
  test(`the_task_page_on_${kind}_is_the_platform_page_bar_with_back_and_the_task_menu_not_v1s_head`, async () => {
    const { client, root } = await start(phone(kind));
    await openTask(client, root, PHONE_TASK, '[data-pane="main"] [data-kit="turns"]', '[data-pane="main"] [data-kit="reply-box"]');
    const bar = root.querySelector('[data-pane="main"] [data-kit="app-bar"][data-part="phone.task.bar"]');
    expect(bar, 'the page TopAppBar in the task’s pane').not.toBeNull();
    expect(bar?.getAttribute('data-kind')).toBe('page');
    expect(bar?.textContent).toContain('Add rate limiting');
    expect(bar?.querySelector('[data-part="phone.task.bar.back"]')?.getAttribute('aria-label'), 'Back names the list').toBe('Tasks');
    expect(bar?.querySelector('[aria-label="More"]'), 'the task’s menu (⋮ / ⋯)').not.toBeNull();
    // v1's head and the details column are not on the phone's page; the ledger and the reply box are, as they are
    expect(root.querySelector('[data-part~="split.head"]')).toBeNull();
    expect(root.querySelector('[data-pane="main"] [data-part="split.details"]'), 'no details column').toBeNull();
    expect(root.querySelector('[data-pane="main"] [data-kit="turns"]')).not.toBeNull();
    expect(root.querySelector('[data-pane="main"] [data-kit="reply-box"]')).not.toBeNull();
  });
}

test('the_task_menu_on_a_phone_holds_v1s_task_actions_and_details_opens_the_side_panel_as_a_sheet', async () => {
  const { client, root } = await start(phone('android'));
  await openTask(client, root, PHONE_TASK);
  (root.querySelector('[data-part="phone.task.bar"] [data-kit="app-bar-actions"] > [aria-label="More"]') as HTMLButtonElement).click();
  await until(() => document.querySelectorAll('[data-kit="menu-item"]').length > 0);
  // the open menu's rows (the list's row menus and the me menu are mounted elsewhere)
  const open = [...document.querySelectorAll('[data-kit="menu"]')].find((m) => m.textContent?.includes('Mark done'));
  const labels = [...(open?.querySelectorAll('[data-kit="menu-item"] [data-kit="menu-item-label"]') ?? [])].map((e) => (e.textContent ?? '').replace(/^Open in .*/, 'Open in'));
  // v1's task actions (the browser's details bar) then its More, from the one menu model; Details first
  const want = ['Details', 'Transcript', 'Rerun…', 'Open in', 'Changes', 'Shell', 'Ship it…', 'Mark done', 'Rename', 'Archive', 'Delete task'];
  for (const w of want) expect(labels, w).toContain(w);
  expect(labels.indexOf('Details')).toBe(0);
  const order = want.slice(0, 8).map((w) => labels.indexOf(w));
  expect(order, 'in v1’s order').toEqual([...order].sort((a, b) => a - b));
  // one control per choice: no second Changes / Shell from the row's menu entries
  expect(labels.filter((l) => l === 'Changes')).toHaveLength(1);
  expect(labels.filter((l) => l === 'Shell')).toHaveLength(1);
  const details = [...(open?.querySelectorAll<HTMLElement>('[data-kit="menu-item"]') ?? [])].find((e) => e.textContent?.includes('Details'));
  details?.click();
  await until(() => q('[data-part~="phone.task.details"] [data-part="split.details"]') !== null);
  const sheet = q('[data-part~="phone.task.details"]');
  expect(sheet?.querySelector('[data-part="split.details"]'), 'the details column, in a sheet').not.toBeNull();
  expect(sheet?.querySelector('[data-kit="it-state"]')?.textContent, 'its state').toBeTruthy();
});

test('rename_on_a_phone_is_a_dialog_from_the_menu_not_an_inline_edit', async () => {
  const { client, root } = await start(phone('ios'));
  await openTask(client, root, PHONE_TASK);
  (root.querySelector('[data-part="phone.task.bar"] [data-kit="app-bar-actions"] > [aria-label="More"]') as HTMLButtonElement).click();
  await until(() => document.querySelectorAll('[data-kit="menu-item"]').length > 0);
  [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].find((e) => e.textContent?.includes('Rename'))?.click();
  await until(() => q('[data-part~="phone.task.rename"] input') !== null);
  expect((q('[data-part~="phone.task.rename"] input') as HTMLInputElement).value).toContain('Add rate limiting');
  expect(q('[data-part="phone.task.rename.save"]')).not.toBeNull();
});

test('a_phones_browser_task_page_keeps_v1s_head_with_back_and_its_details_bar', async () => {
  const { client, root } = await start(nullPlatform({ mobile: true }));
  await openTask(client, root, '[data-part~="split.head"] [data-part="split.back"]', '[data-part="split.details"] [data-part="split.actions"]');
  expect(document.documentElement.hasAttribute('data-platform')).toBe(false);
  expect(root.querySelector('[data-part~="split.head"] [data-part="split.back"]'), 'v1’s Back').not.toBeNull();
  expect(root.querySelector('[data-part="split.details"] [data-part="split.actions"]'), 'v1’s details bar and its actions').not.toBeNull();
  expect(root.querySelector('[data-part="phone.task.bar"]')).toBeNull();
});

for (const kind of ['android', 'ios'] as const) {
  test(`new_task_on_${kind}_is_the_platform_compose_page_with_its_way_out_and_send_and_every_v1_composer_feature`, async () => {
    const { root } = await start(phone(kind));
    await until(() => root.querySelector('[data-part~="phone.tasks.new"]') !== null);
    (root.querySelector('[data-part~="phone.tasks.new"]') as HTMLElement).click();
    const PARTS = ['launcher.input', 'launcher.harness', 'launcher.projects', 'launcher.attach', 'launcher.team', 'launcher.goal', 'launcher.where', 'launcher.send'];
    // the bar and the whole prompt box under it
    await until(() => root.querySelector('[data-part="phone.new.bar"] [data-part="phone.new.bar.confirm"]') !== null && PARTS.every((part) => root.querySelector(`[data-part~="${part}"]`) !== null));
    const bar = root.querySelector('[data-part="phone.new.bar"]');
    expect(bar?.getAttribute('data-kind')).toBe('compose');
    expect(bar?.textContent).toContain('New task');
    const out = bar?.querySelector('[data-part="phone.new.bar.back"]');
    // Android: the full-screen dialog's ×; iOS: the sheet's Cancel
    if (kind === 'android') expect(out?.getAttribute('aria-label')).toBe('Close');
    else expect(out?.textContent).toBe('Cancel');
    const send = bar?.querySelector<HTMLButtonElement>('[data-part="phone.new.bar.confirm"]');
    expect(send?.textContent).toBe('Send');
    expect(send?.disabled, 'nothing typed yet').toBe(true);
    // over the list, which stays behind it; v1's Back and greeting bar are not drawn
    expect(root.querySelector('[data-kit="frame"]')?.hasAttribute('data-behind')).toBe(true);
    expect(root.querySelector('[data-part="split.back"]')).toBeNull();
    // the prompt box keeps every v1 feature: harness pill and picker, folder, attachments, delegate, goal, the place
    for (const part of PARTS)
      expect(root.querySelector(`[data-part~="${part}"]`), part).not.toBeNull();
    const input = root.querySelector<HTMLTextAreaElement>('[data-part~="launcher.input"]');
    if (input) {
      input.value = 'Add a rate limit';
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
    await until(() => bar?.querySelector<HTMLButtonElement>('[data-part="phone.new.bar.confirm"]')?.disabled === false);
    expect(bar?.querySelector<HTMLButtonElement>('[data-part="phone.new.bar.confirm"]')?.disabled, 'typed: Send is on').toBe(false);
    // Close / Cancel goes back to the list and keeps the draft
    (out as HTMLElement).click();
    await until(() => root.querySelector('[data-kit="frame"]')?.getAttribute('data-page') === 'list');
    expect(root.querySelector('[data-kit="frame"]')?.getAttribute('data-page')).toBe('list');
  });
}

// r291: the phone sheet is the prompt box alone, focused; Home's greeting, Needs you, tips, keys and feedback line
// stay in the browser's Home only (the phone apps' Tasks list is where tasks are)
for (const kind of ['android', 'ios'] as const) {
  test(`the_new_task_sheet_on_${kind}_holds_only_the_composer_focused_with_no_home_list_or_tips`, async () => {
    const { root } = await start(phone(kind));
    await until(() => root.querySelector('[data-part~="phone.tasks.new"]') !== null);
    (root.querySelector('[data-part~="phone.tasks.new"]') as HTMLElement).click();
    // the sheet as it opened: the caret in the prompt (the last thing it does), Start from an issue drawn
    await until(
      () =>
        root.querySelector('[data-part="phone.new.bar"]') !== null &&
        document.activeElement === root.querySelector('[data-part~="launcher.input"]') &&
        root.querySelector('[data-part="split.home"] [data-part~="launcher.issue"]') !== null,
    );
    const home = root.querySelector('[data-part="split.home"]');
    expect(home?.querySelector('[data-part~="launcher.input"]'), 'the prompt').not.toBeNull();
    expect(home?.querySelector('[data-part~="launcher.projects"]'), 'the project selector').not.toBeNull();
    for (const part of ['split.greeting', 'split.next', 'tips', 'home.feedback'])
      expect(home?.querySelector(`[data-part~="${part}"]`), `${part} is not in the sheet`).toBeNull();
    // what the sheet holds besides the bar: the prompt box and its own popups, nothing of Home's page
    const kids = [...(home?.children ?? [])].map((e) => e.getAttribute('data-kit'));
    expect(kids.filter((k) => k !== null)).toEqual(['sh-launch']);
    // no recipes: the phones drop the empty Recipes list (v1 responsive.css) and keep Start from an issue
    expect(home?.querySelector('[data-part="launcher.recipes-wrap"]')?.hasAttribute('data-empty')).toBe(true);
    expect(home?.querySelector('[data-part~="launcher.issue"]'), 'Start from an issue').not.toBeNull();
    // the caret is in the prompt, as the sheet opens (the keyboard with it)
    expect(document.activeElement).toBe(root.querySelector('[data-part~="launcher.input"]'));
  });
}

test('new_task_in_a_phones_browser_keeps_v1s_back_and_greeting_and_no_compose_bar', async () => {
  const { root } = await start(nullPlatform({ mobile: true }));
  await until(() => root.querySelector('[data-part~="sidebar.new"], [data-part~="list.new"]') !== null);
  (root.querySelector('[data-part~="sidebar.new"], [data-part~="list.new"]') as HTMLElement).click();
  await until(() => root.querySelector('[data-part="split.back"]') !== null);
  expect(root.querySelector('[data-part="split.home"] [data-part="split.back"]'), 'v1’s Back').not.toBeNull();
  expect(root.querySelector('[data-part="phone.new.bar"]')).toBeNull();
  // r291: the browser keeps v1's whole Home under the box (the greeting, Needs you, the tips)
  for (const part of ['split.greeting', 'split.next', 'tips']) expect(root.querySelector(`[data-part="split.home"] [data-part~="${part}"]`), part).not.toBeNull();
});

for (const kind of ['android', 'ios'] as const) {
  test(`search_on_${kind}_is_the_platform_search_view_over_the_same_task_search_with_its_way_back`, async () => {
    const { root } = await start(phone(kind));
    // the bar, and the tasks it searches listed
    await until(() => root.querySelector('[data-part="phone.tasks.bar"]') !== null && root.querySelector('[data-pane="list"]')?.textContent?.includes('Add rate limiting') === true);
    // Android: the bar's search icon; iOS: the search field under the large title
    const opener = kind === 'android' ? '[data-part="phone.tasks.bar.search"]' : '[data-part="phone.tasks.bar.search-field"]';
    expect(root.querySelector(opener), opener).not.toBeNull();
    (root.querySelector(opener) as HTMLElement).click();
    await until(() => q('[data-kit="pal"] [data-kit="pal-row"]') !== null);
    // the desktop's task search (the palette), its rows the same tasks
    expect(q('[data-kit="pal"]')?.textContent).toContain('Add rate limiting');
    const back = q(kind === 'android' ? '[data-part="shell.palette.back"]' : '[data-part="shell.palette.cancel"]');
    expect(back, 'Back (Android) / Cancel (iOS)').not.toBeNull();
    if (kind === 'ios') expect(back?.textContent).toBe('Cancel');
    back?.click();
    await until(() => q('[data-kit="pal"]') === null);
    expect(q('[data-kit="pal"]'), 'closed').toBeNull();
  });
}

test('search_in_a_phones_browser_is_v1s_palette_with_no_back_or_cancel', async () => {
  const { client } = await start(nullPlatform({ mobile: true }));
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: null } });
  await until(() => q('[data-kit="pal"]') !== null);
  expect(q('[data-part="shell.palette.back"]')).toBeNull();
  expect(q('[data-part="shell.palette.cancel"]')).toBeNull();
  expect(q('[data-kit="pal-foot"]'), 'v1’s keys foot').not.toBeNull();
});

// The screens' lazy chunks (each look's halves, the phone task page and its menu, the details sheet), compiled while
// vitest collects this file, off the tests' clock (the dynamic-import-in-test rule's reason): under the full suite's
// load a cold chunk outlasted the task menu test's waits. Each look opens the task, its menu and Details once.
for (const kind of ['android', 'ios'] as const) {
  await loadPlatformStyles(kind);
  const { client, root } = await start(phone(kind));
  const task = (): { id: string } | undefined => client.entities.list<{ id: string; title: string }>('session')().find((r) => r.title.startsWith('Add rate limiting'));
  await until(() => task() !== undefined, 60_000);
  client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
  await until(() => root.querySelector(PHONE_TASK) !== null, 60_000);
  root.querySelector<HTMLButtonElement>(PHONE_TASK)?.click();
  await until(() => document.querySelector('[data-kit="menu-item"]') !== null, 60_000);
  [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].find((e) => e.textContent?.includes('Details'))?.click();
  await until(() => q('[data-part~="phone.task.details"] [data-part="split.details"]') !== null, 60_000);
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
}
