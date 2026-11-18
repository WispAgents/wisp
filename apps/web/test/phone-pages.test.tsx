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

/** `computers`: the seed with a project on this computer (a git repo), so New task starts in a folder */
async function start(platform: Platform, o: { computers?: boolean } = {}) {
  const mock = createMockPeer({ epoch: 'e1', ...(o.computers && { computers: true }) });
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
const labelOf = (e: Element): string => (e.querySelector('[data-kit="menu-item-label"]')?.textContent ?? '').replace(/^Open in .*/, 'Open in');
/** what the open menu reaches (QA r2 #5: on a phone its longer sections are submenus): its own rows, and each
 *  submenu's rows, drilled into and back out of */
async function reach(menu: Element): Promise<{ top: string[]; inside: Record<string, string[]> }> {
  const rows = (): Element[] => [...menu.querySelectorAll('[data-kit="menu-item"]:not([data-back])')];
  const top = rows().map(labelOf);
  const inside: Record<string, string[]> = {};
  for (const name of rows()
    .filter((e) => e.hasAttribute('data-sub'))
    .map(labelOf)) {
    rows()
      .find((e) => e.hasAttribute('data-sub') && labelOf(e) === name)
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await until(() => menu.hasAttribute('data-drilled'));
    inside[name] = rows().map(labelOf);
    menu.querySelector<HTMLElement>('[data-kit="menu-item"][data-back]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await until(() => !menu.hasAttribute('data-drilled'));
  }
  return { top, inside };
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
  // iOS's ⋯ menu (Android's is the same entries in an M3 bottom sheet: phone-fewer.test.tsx)
  const { client, root } = await start(phone('ios'));
  await openTask(client, root, PHONE_TASK);
  (root.querySelector('[data-part="phone.task.bar"] [data-kit="app-bar-actions"] > [aria-label="More"]') as HTMLButtonElement).click();
  await until(() => document.querySelectorAll('[data-kit="menu-item"]').length > 0);
  // the open menu (the list's row menus and the me menu are mounted elsewhere)
  const open = [...document.querySelectorAll('[data-kit="menu"]')].find((m) => m.textContent?.includes('Mark done'));
  expect(open, 'the task’s menu').toBeDefined();
  const { top, inside } = await reach(open as Element);
  // three groups, nine rows at most (UX 2026-11-18 phone #6, QA r6 #6; HIG ≈3 groups): act (Ship it…, Mark done), work
  // on it (Rerun…, This task ›, Show ›, Keep and copy ›), put it away (Save for later, Archive, Delete)
  expect(top.slice(0, 2)).toEqual(['Ship it…', 'Mark done']);
  for (const w of ['Rerun…', 'This task', 'Show', 'Archive', 'Delete task']) expect(top, w).toContain(w);
  expect(top.indexOf('Rerun…')).toBeLessThan(top.indexOf('This task'));
  expect(top.at(-1)).toBe('Delete task');
  expect(top.length, 'a menu that fits a phone').toBeLessThanOrEqual(9);
  // every v1 action still reachable, once: Details, Rename, Open in in This task; Transcript, Changes, Shell in Show
  for (const w of ['Details', 'Rename', 'Open in']) expect(inside['This task'], w).toContain(w);
  // what you take away is in Keep and copy (Fork…, the copies), never in This task
  expect([...(inside['Keep and copy'] ?? []), ...top], 'Fork…').toContain('Fork…');
  expect(inside['This task'], 'no Fork in This task').not.toContain('Fork…');
  for (const w of ['Transcript', 'Changes', 'Shell']) expect(inside['Show'], `${w} in ${JSON.stringify(inside)}`).toContain(w);
  const all = [...top, ...Object.values(inside).flat()];
  expect(all.filter((l) => l === 'Changes')).toHaveLength(1);
  expect(all.filter((l) => l === 'Shell')).toHaveLength(1);
  // Details from This task
  [...((open as Element).querySelectorAll<HTMLElement>('[data-kit="menu-item"][data-sub]:not([data-back])') ?? [])]
    .find((e) => labelOf(e) === 'This task')
    ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  await until(() => (open as Element).hasAttribute('data-drilled'));
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
  // Rename is in This task (QA r2 #5: the phone's menu folds its longer sections)
  [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"][data-sub]')].find((e) => e.textContent?.includes('This task'))?.click();
  await until(() => [...document.querySelectorAll('[data-kit="menu-item"]')].some((e) => e.textContent?.includes('Rename')));
  [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].find((e) => e.textContent?.includes('Rename'))?.click();
  // the one Rename task dialog, as the desktop's head and rows open it (QA round 15 #2)
  await until(() => q('[data-part~="list.rename"] input') !== null);
  expect((q('[data-part~="list.rename"] input') as HTMLInputElement).value).toContain('Add rate limiting');
  expect(q('[data-part~="list.rename.save"]')).not.toBeNull();
});

test('a_phones_browser_task_page_keeps_v1s_head_with_back_and_its_details_bar', async () => {
  const { client, root } = await start(nullPlatform({ mobile: true }));
  await openTask(client, root, '[data-part~="split.head"] [data-part="split.back"]', '[data-part="split.details"]');
  expect(document.documentElement.hasAttribute('data-platform')).toBe(false);
  expect(root.querySelector('[data-part~="split.head"] [data-part="split.back"]'), 'v1’s Back').not.toBeNull();
  expect(root.querySelector('[data-part="split.details"]'), 'v1’s details bar').not.toBeNull();
  // its actions are the head's one bar now (docs/ux/2026-11-18-desktop.md #4): Ship, Done, ⋯
  expect(root.querySelector('[data-part~="split.head"] [data-part="split.more"]'), 'the head’s actions').not.toBeNull();
  expect(root.querySelector('[data-part="phone.task.bar"]')).toBeNull();
});

for (const kind of ['android', 'ios'] as const) {
  test(`new_task_on_${kind}_is_the_platform_compose_page_with_its_way_out_and_send_and_every_v1_composer_feature`, async () => {
    const { client, root } = await start(phone(kind));
    // the mock's tasks in first: with one of yours the bar is full, its tail folded into More (first-run.ts)
    await until(() => client.entities.list('session')().length > 0);
    await until(() => root.querySelector('[data-part~="phone.tasks.new"]') !== null);
    (root.querySelector('[data-part~="phone.tasks.new"]') as HTMLElement).click();
    // Goal, Worktree, Start later, Start from an issue and Recipes are in the bar's More on a phone (QA r2 #2)
    const PARTS = ['launcher.input', 'launcher.harness', 'launcher.projects', 'launcher.attach', 'launcher.team', 'launcher.more', 'launcher.where', 'launcher.send'];
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
    // one primary action, named for what it does (UX 2026-11-18 phone #5)
    expect(send?.textContent).toBe('Start');
    expect(send?.disabled, 'nothing typed yet').toBe(true);
    // over the list, which stays behind it; v1's Back and greeting bar are not drawn
    expect(root.querySelector('[data-kit="frame"]')?.hasAttribute('data-behind')).toBe(true);
    expect(root.querySelector('[data-part="split.back"]')).toBeNull();
    // the prompt box keeps every v1 feature: harness pill and picker, folder, attachments, delegate, goal, the place
    for (const part of PARTS)
      expect(root.querySelector(`[data-part~="${part}"]`), part).not.toBeNull();
    // … and the rest one tap away in More: the goal, the start time, an issue, the recipes
    // the open menu More drew (portaled: the one holding Start from an issue)
    const moreRows = (): string[] => {
      const menu = [...document.querySelectorAll('[data-kit="menu"]')].find((m) => m.textContent?.includes('Start from an issue'));
      return [...(menu?.querySelectorAll('[role^="menuitem"]') ?? [])].map((e) => (e.textContent ?? '').trim());
    };
    const more = (): HTMLElement | null => root.querySelector('[data-part~="launcher.more"]');
    expect(more()?.getAttribute('aria-haspopup'), 'More is a menu, not the first run’s More options').toBe('menu');
    more()?.click();
    await until(() => moreRows().some((l) => l.startsWith('Start later')));
    for (const w of ['Goal', 'Start later', 'Start from an issue', 'Recipes']) expect(moreRows().some((l) => l.startsWith(w)), w).toBe(true);
    // no folder picked (the mock's New task): no git repo, so no Worktree (v1 paintScratch); with one it folds in too
    // (packages/shell/test/phone-bar.test.ts)
    expect(root.querySelector('[data-part~="launcher.projects"]')?.textContent).toContain('No folder');
    expect(moreRows().some((l) => l.startsWith('Worktree') || l.startsWith('In place')), 'no Worktree without a folder').toBe(false);
    // one state for both Sends (QA r2 #12): the round one is off while the header's is
    expect(root.querySelector<HTMLButtonElement>('[data-part="launcher.send"]')?.disabled, 'the round Send, nothing typed').toBe(true);
    const input = root.querySelector<HTMLTextAreaElement>('[data-part~="launcher.input"]');
    if (input) {
      input.value = 'Add a rate limit';
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
    await until(() => bar?.querySelector<HTMLButtonElement>('[data-part="phone.new.bar.confirm"]')?.disabled === false);
    expect(bar?.querySelector<HTMLButtonElement>('[data-part="phone.new.bar.confirm"]')?.disabled, 'typed: Send is on').toBe(false);
    expect(root.querySelector<HTMLButtonElement>('[data-part="launcher.send"]')?.disabled, 'typed: the round Send is on too').toBe(false);
    // Close / Cancel goes back to the list and keeps the draft
    (out as HTMLElement).click();
    await until(() => root.querySelector('[data-kit="frame"]')?.getAttribute('data-page') === 'list');
    expect(root.querySelector('[data-kit="frame"]')?.getAttribute('data-page')).toBe('list');
  });
}

// QA r2 #2: with a folder picked (a git repo) Worktree is in More too, a toggle showing its state, and a tap flips it
for (const kind of ['android', 'ios'] as const) {
  test(`new_task_on_${kind}_in_a_folder_has_worktree_in_more_with_its_state`, async () => {
    const { client, root } = await start(phone(kind), { computers: true });
    await until(() => client.entities.list('session')().length > 0);
    await until(() => root.querySelector('[data-part~="phone.tasks.new"]') !== null);
    (root.querySelector('[data-part~="phone.tasks.new"]') as HTMLElement).click();
    await until(() => root.querySelector('[data-part~="launcher.projects"]')?.textContent?.includes('wisp') === true);
    expect(root.querySelector('[data-part~="launcher.projects"]')?.textContent, 'the folder').toContain('wisp');
    expect(root.querySelector('[data-part~="launcher.worktree"]'), 'not in the bar: in More').toBeNull();
    const more = root.querySelector<HTMLElement>('[data-part~="launcher.more"]');
    expect(more?.getAttribute('aria-haspopup')).toBe('menu');
    more?.click();
    // in the open menu only: a closed menu stays in the page, so "the row is gone" on every menu never came true and
    // the wait below ran out its whole 4 s each time (CI 44273734971 timed out on that; the menu closes in a frame)
    const row = (): HTMLElement | undefined =>
      [...document.querySelectorAll<HTMLElement>('[data-kit="menu"][data-state="open"] [role^="menuitem"]')].find((e) => /^(Worktree|In place)/.test((e.textContent ?? '').trim()));
    await until(() => row() !== undefined);
    const before = row();
    expect(before, 'Worktree in More').toBeDefined();
    // a toggle: its state on the row, the kit's check at the end (on = a worktree), its words following it
    const on = (e: HTMLElement | undefined): boolean => (e?.querySelector('[data-icon="check"]') ?? null) !== null;
    const was = on(before);
    expect((before?.textContent ?? '').trim().startsWith(was ? 'Worktree' : 'In place'), 'its words follow its state').toBe(true);
    // the tap flips it (the worktree setting, optimistic), seen on the bar's More as reopened
    before?.click();
    await until(() => row() === undefined);
    more?.click();
    await until(() => row() !== undefined && on(row()) !== was);
    expect(on(row()), 'flipped').toBe(!was);
    expect((row()?.textContent ?? '').trim().startsWith(was ? 'In place' : 'Worktree'), 'and its words').toBe(true);
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
  // r291: the browser keeps v1's whole Home under the box (the greeting, the tips; Needs you only when a task needs
  // you, as v1: QA r2 #2 hid an empty "Needs you 0")
  for (const part of ['split.greeting', 'tips']) expect(root.querySelector(`[data-part="split.home"] [data-part~="${part}"]`), part).not.toBeNull();
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
