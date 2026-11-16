// @vitest-environment happy-dom
// ADR-0065 step 13's last three (design/mobile.md §A.4), through the real client, shell and list plugin against the
// mock peer: the Tasks list's selection mode (Android's contextual top app bar, iOS's edit mode with its bottom
// toolbar; the bulk actions optimistic, a refusal rolled back with a toast), the iOS computer filter as a pull-down
// from the title, and a task page's Next task / Previous task (the twins of the head's swipe) in the list's order.
import { afterEach, expect, test } from 'vitest';
import { createClient, memoryHistory, peerId, type Client, type EntityKey } from '@wisp/client';
import { commandErrorToast, createShell, nullPlatform, type Platform, type ShellContext, type ShellWindow } from '@wisp/shell';
import { createToaster, loadPlatformStyles } from '@wisp/ui';
import { createMockPeer, type MockPeer } from '../dev/mock-peer.ts';
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

const PLUGINS = new Set(['wisp.shell-list', 'wisp.shell-task']);

async function start(platform: Platform, o: { computers?: boolean; history?: ReturnType<typeof memoryHistory> } = {}): Promise<{ root: HTMLElement; mock: MockPeer; client: Client; shell: ShellContext }> {
  const mock = createMockPeer({ epoch: 'e1', ...(o.computers && { computers: true }) });
  const home = peerId('local');
  const toaster = createToaster();
  // as the app does: a refused command rolls back with a red toast
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390, onError: commandErrorToast(toaster), ...(o.history && { history: o.history }) });
  const shell = createShell({ client, toaster, platform, home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def && PLUGINS.has(def.id)) shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  await until(() => root.querySelectorAll('[data-pane="list"] [data-part="card"][data-id]').length > 2);
  // the look's chunks the shell started loading as it set data-platform (the overlays' phone halves: the long-press
  // sheet), landed before the test presses anything, as on a phone they land with the first screen (B22: a cold
  // chunk on a busy runner outlasted the long-press's wait)
  await loadPlatformStyles(platform.kind === 'ios' ? 'ios' : 'android');
  return { root, mock, client, shell: shell.context };
}

const phone = (kind: 'android' | 'ios'): Platform => nullPlatform({ kind, phone: true, mobile: true });
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const cards = (root: ParentNode): HTMLElement[] => [...root.querySelectorAll<HTMLElement>('[data-pane="list"] [data-part="card"][data-id]')];
const rowOf = (card: HTMLElement): HTMLElement => card.querySelector<HTMLElement>('[data-kit="list-row"]') as HTMLElement;
const title = (): string => q('[data-part="phone.tasks.select.title"]')?.textContent ?? '';
/** a button by its words, out of the toolbars' hidden measuring copies */
const button = (scope: ParentNode, label: string): HTMLButtonElement | undefined =>
  [...scope.querySelectorAll<HTMLButtonElement>('button')].find((b) => !b.closest('[data-kit="toolbar-measure"]') && (b.getAttribute('aria-label') === label || b.textContent?.trim() === label));
const session = (client: Client, key: string): { archived: boolean; state: string } | undefined => client.entities.peek(`session`, key as EntityKey) as never;

/** a long-press on a row: what Swipe's onLongPress does (the row's contextmenu, at the row) */
async function longPress(card: HTMLElement): Promise<void> {
  const r = rowOf(card);
  const b = r.getBoundingClientRect();
  r.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: b.left, clientY: b.bottom }));
  await until(() => q('[data-kit="sheet-item"], [data-kit="context-item"]') !== null);
}

test('on_android_a_rows_long_press_sheet_offers_select_first_and_it_opens_the_contextual_bar_with_that_row_picked', async () => {
  const { root } = await start(phone('android'));
  const first = cards(root)[0] as HTMLElement;
  await longPress(first);
  const items = [...document.querySelectorAll<HTMLElement>('[data-kit="sheet-item"]')];
  expect(items[0]?.dataset['value'], 'Select, first').toBe('menu:select');
  items[0]?.click();
  await until(() => q('[data-part="phone.tasks.select"]') !== null);
  expect(q('[data-part="phone.tasks.bar"]'), 'the Tasks bar steps aside').toBeNull();
  expect(title()).toBe('1 selected');
  expect(rowOf(first).getAttribute('aria-pressed')).toBe('true');
  expect(q('[data-part="phone.tasks.new"]'), 'no FAB while selecting').toBeNull();
  expect(q('[data-part="phone.tasks.select.exit"]')?.getAttribute('aria-label'), 'its × leaves').toBe('Close selection');
});

test('in_selection_a_tap_toggles_rows_and_unpicking_the_last_on_android_leaves_it', async () => {
  const { root, shell } = await start(phone('android'));
  const [a, b] = cards(root) as [HTMLElement, HTMLElement];
  shell.listSelect?.start(a.dataset['id']);
  await until(() => q('[data-part="phone.tasks.select"]') !== null);
  rowOf(b).click();
  await until(() => title() === '2 selected');
  expect(title()).toBe('2 selected');
  expect(rowOf(b).hasAttribute('data-picked')).toBe(true);
  rowOf(a).click();
  rowOf(b).click();
  await until(() => q('[data-part="phone.tasks.bar"]') !== null);
  expect(q('[data-part="phone.tasks.select"]'), 'left with nothing picked').toBeNull();
  expect(rowOf(a).hasAttribute('data-picking'), 'rows back to themselves').toBe(false);
});

test('on_ios_select_in_the_bar_is_edit_mode_in_the_same_bar_with_select_all_done_and_the_bulk_actions_in_a_bottom_toolbar', async () => {
  const { root, shell } = await start(phone('ios'));
  // the bar's Select (the twin of the long-press)
  expect(button(q('[data-part="phone.tasks.bar"]') as HTMLElement, 'Select') ?? q('[data-part="phone.tasks.bar"] [aria-label="More"]'), 'Select in the bar').toBeDefined();
  const bar = q('[data-part="phone.tasks.bar"]') as HTMLElement;
  const large = bar.querySelector('[data-kit="app-bar-large"]');
  shell.listSelect?.start();
  await until(() => q('[data-part="phone.tasks.select.title"]') !== null);
  // edit mode is the Tasks bar itself (iOS Mail, Files): the same node, its large title kept, so the list doesn't move
  expect(q('[data-part="phone.tasks.bar"]'), 'the same bar, not a new one').toBe(bar);
  expect(bar.hasAttribute('data-selecting')).toBe(true);
  expect(bar.querySelector('[data-kit="app-bar-large"]'), 'the large title stays').toBe(large);
  expect(document.querySelectorAll('[data-kit="app-bar"]').length, 'one bar').toBe(1);
  expect(title()).toBe('Select Items');
  expect(button(bar, 'Select All')).toBeDefined();
  expect(button(bar, 'Done')).toBeDefined();
  expect(q('[data-kit="select-toolbar"]'), 'the bottom toolbar').not.toBeNull();
  // nothing picked: every bulk action is off
  const tools = q('[data-kit="select-toolbar"]') as HTMLElement;
  for (const l of ['Mark done', 'Archive', 'Delete']) expect(button(tools, l)?.disabled, l).toBe(true);
  button(bar, 'Select All')?.click();
  await until(() => title() === `${cards(root).length} Selected`);
  const n = cards(root).length;
  expect(title()).toBe(`${n} Selected`);
  expect(button(bar, 'Deselect All'), 'Select All turns into its undo').toBeDefined();
  // unpicking every row keeps iOS in edit mode, until Done
  button(bar, 'Deselect All')?.click();
  await until(() => title() === 'Select Items');
  expect(title()).toBe('Select Items');
  button(bar, 'Done')?.click();
  await until(() => !bar.hasAttribute('data-selecting'));
  expect(q('[data-part="phone.tasks.bar"]'), 'still the same bar').toBe(bar);
  expect(q('[data-kit="select-toolbar"]')).toBeNull();
});

test('on_android_the_contextual_bar_takes_the_top_app_bars_place_one_bar_at_a_time', async () => {
  const { shell } = await start(phone('android'));
  const frameTop = q('[data-part="phone.tasks.bar"]')?.parentElement;
  shell.listSelect?.start();
  await until(() => q('[data-part="phone.tasks.select"]') !== null);
  expect(q('[data-part="phone.tasks.bar"]'), 'the top app bar gave way').toBeNull();
  expect(q('[data-part="phone.tasks.select"]')?.parentElement, 'in its slot').toBe(frameTop);
  expect(document.querySelectorAll('[data-kit="app-bar"]').length).toBe(1);
  // leaving: the top app bar is back in the same slot, the contextual bar gone
  shell.listSelect?.exit();
  await until(() => q('[data-part="phone.tasks.bar"]') !== null);
  expect(q('[data-part="phone.tasks.bar"]')?.parentElement).toBe(frameTop);
  expect(q('[data-part="phone.tasks.select"]')).toBeNull();
  expect(document.querySelectorAll('[data-kit="app-bar"]').length).toBe(1);
});

test('a_bulk_archive_changes_every_picked_row_at_once_and_a_refusal_rolls_it_back_with_a_toast', async () => {
  const { root, client, mock, shell } = await start(phone('android'));
  const ready = cards(root)
    .map((c) => c.dataset['id'] ?? '')
    .filter((k) => {
      const s = session(client, k);
      return s && !s.archived && s.state !== 'running' && s.state !== 'waiting';
    });
  expect(ready.length, 'two finished tasks in the mock').toBeGreaterThan(1);
  const [a, b] = ready as [string, string];
  shell.listSelect?.start(a);
  shell.listSelect?.toggle(b);
  await until(() => q('[data-part="phone.tasks.select"]') !== null);
  mock.failNext('runtime.archive');
  button(q('[data-part="phone.tasks.select"]') as HTMLElement, 'Archive')?.click();
  // the same frame: both archived, the bar gone
  expect(session(client, a)?.archived).toBe(true);
  expect(session(client, b)?.archived).toBe(true);
  expect(q('[data-part="phone.tasks.select"]')).toBeNull();
  // the peer refused the first: it comes back, with the client's toast; the second stays archived
  await until(() => session(client, a)?.archived === false);
  expect(session(client, a)?.archived).toBe(false);
  expect(session(client, b)?.archived).toBe(true);
  await until(() => document.body.textContent?.includes('refused') === true);
  expect(document.body.textContent).toContain('refused');
});

test('a_bulk_delete_asks_first_with_the_count', async () => {
  const { root, client, shell } = await start(phone('ios'));
  const [a, b] = cards(root).map((c) => c.dataset['id'] ?? '') as [string, string];
  shell.listSelect?.start(a);
  shell.listSelect?.toggle(b);
  await until(() => q('[data-kit="select-toolbar"]') !== null);
  button(q('[data-kit="select-toolbar"]') as HTMLElement, 'Delete')?.click();
  await until(() => q('[data-part~="phone.tasks.select.ask"]') !== null);
  expect(q('[data-part~="phone.tasks.select.ask"]')?.textContent).toContain('Delete 2 tasks from wisp?');
  expect(session(client, a), 'nothing yet').toBeDefined();
  q('[data-part="phone.tasks.select.ask.ok"]')?.click();
  expect(session(client, a), 'gone at once').toBeUndefined();
  expect(session(client, b)).toBeUndefined();
});

test('on_ios_the_computer_filter_is_a_pull_down_from_the_title_with_the_shown_one_checked_not_a_sheet', async () => {
  const ios = await start(phone('ios'));
  const trigger = ios.root.querySelector<HTMLElement>('[data-part="phone.tasks.bar.title"] [data-kit="app-bar-title-button"]');
  expect(trigger?.getAttribute('aria-haspopup'), 'a menu button').toBe('menu');
  trigger?.click();
  await until(() => [...document.querySelectorAll('[data-kit="menu-item"]')].some((e) => e.textContent?.includes('mock')));
  const choices = [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].filter((e) => e.closest('[data-part~="phone.tasks.bar.title.menu"]'));
  expect(choices.map((e) => e.textContent?.trim())).toEqual(['All', 'mock']);
  expect(choices.find((e) => e.textContent?.includes('All'))?.querySelector('[data-kit="icon"]:last-child'), 'the one shown, checked').not.toBeNull();
  expect(choices.find((e) => e.textContent?.includes('mock'))?.querySelectorAll('[data-kit="icon"]').length, 'the other not').toBe(0);
  expect(q('[data-part~="phone.tasks.computers"] [data-kit="radio"]'), 'no sheet').toBeNull();
  choices.find((e) => e.textContent?.includes('mock'))?.click();
  await until(() => ios.root.querySelector('[data-part="phone.tasks.bar.title"]')?.textContent?.includes('mock') === true);
  expect(ios.root.querySelector('[data-part="phone.tasks.bar.title"]')?.textContent, 'the pick is the title').toContain('mock');
});

test('on_android_the_computer_filter_keeps_its_sheet', async () => {
  const android = await start(phone('android'));
  const t = android.root.querySelector<HTMLElement>('[data-part="phone.tasks.bar.title"] [data-kit="app-bar-title-button"]');
  expect(t?.hasAttribute('aria-haspopup'), 'Android: a plain button').toBe(false);
  t?.click();
  await until(() => q('[data-part~="phone.tasks.computers"] [data-kit="radio"]') !== null);
  expect(q('[data-part~="phone.tasks.computers"] [data-kit="radio"]'), 'its sheet of choices').not.toBeNull();
  expect(q('[data-part~="phone.tasks.bar.title.menu"]'), 'no pull-down').toBeNull();
});

async function openMenu(): Promise<HTMLElement[]> {
  await until(() => q('[data-part="phone.task.bar"] [data-kit="app-bar-actions"] > [aria-label="More"]') !== null);
  (q('[data-part="phone.task.bar"] [data-kit="app-bar-actions"] > [aria-label="More"]') as HTMLButtonElement).click();
  await until(() => [...document.querySelectorAll('[data-kit="menu-item"]')].some((e) => e.textContent?.includes('Next task')));
  return [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].filter((e) => /Next task|Previous task/.test(e.textContent ?? ''));
}

for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_next_task_and_previous_task_in_the_menu_step_through_the_lists_order_disabled_at_its_ends`, async () => {
    const { root, client, shell } = await start(phone(kind));
    const order = shell.listOrder?.keys() ?? [];
    expect(order.length).toBeGreaterThan(2);
    expect(cards(root).map((c) => c.dataset['id'])).toEqual(expect.arrayContaining(order.slice(0, 3)));
    client.nav.navigate({ t: 'openTask', id: order[0] as never });
    await until(() => q('[data-part="phone.task.bar"]') !== null);
    // the head answers the swipe, its twins named on it
    expect(q('[data-part="phone.task.bar"]')?.getAttribute('data-gesture')).toContain('task.head-swipe');
    let [next, prev] = await openMenu();
    expect(next?.textContent).toContain('Next task');
    expect(prev?.hasAttribute('data-disabled') || prev?.getAttribute('aria-disabled') === 'true', 'the first task has no previous').toBe(true);
    next?.click();
    await until(() => client.nav.nav().main.kind === 'task' && String((client.nav.nav().main as { id: string }).id) === order[1]);
    expect(String((client.nav.nav().main as { id: string }).id)).toBe(order[1]);
    [next, prev] = await openMenu();
    prev?.click();
    await until(() => String((client.nav.nav().main as { id: string }).id) === order[0]);
    expect(String((client.nav.nav().main as { id: string }).id)).toBe(order[0]);
  });
}

const openId = (client: Client): string => {
  const m = client.nav.nav().main;
  return m.kind === 'task' ? String(m.id) : m.kind;
};

for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_back_after_two_steps_returns_to_tasks_not_through_each_stepped_task`, async () => {
    const history = memoryHistory('/');
    const { client, shell } = await start(phone(kind), { history });
    const order = shell.listOrder?.keys() ?? [];
    expect(order.length).toBeGreaterThan(2);
    client.nav.navigate({ t: 'openTask', id: order[0] as never });
    await until(() => q('[data-part="phone.task.bar"]') !== null);
    const entries = history.entries.length;
    // step twice with Next task (the head swipe calls the same stepTo)
    (await openMenu())[0]?.click();
    await until(() => openId(client) === order[1]);
    (await openMenu())[0]?.click();
    await until(() => openId(client) === order[2]);
    expect(openId(client)).toBe(order[2]);
    expect(history.entries.length, 'a step adds no history entry').toBe(entries);
    // the platform's Back (Android's Back, iOS's edge swipe: the history's)
    history.back();
    await until(() => openId(client) === 'home');
    expect(openId(client), 'Back lands on Tasks').toBe('home');
    await until(() => q('[data-part="phone.tasks.bar"]') !== null);
    expect(q('[data-part="phone.tasks.bar"]'), 'the Tasks bar').not.toBeNull();
    // and the bar's Back, after two steps again
    client.nav.navigate({ t: 'openTask', id: order[0] as never });
    await until(() => q('[data-part="phone.task.bar"]') !== null);
    (await openMenu())[0]?.click();
    await until(() => openId(client) === order[1]);
    (await openMenu())[0]?.click();
    await until(() => openId(client) === order[2]);
    q('[data-part="phone.task.bar.back"]')?.click();
    await until(() => openId(client) === 'home');
    expect(openId(client)).toBe('home');
  });
}

test('a_filter_that_changes_the_lists_order_moves_next_task_with_it', async () => {
  const { client, shell } = await start(phone('android'));
  const order = shell.listOrder?.keys() ?? [];
  client.nav.navigate({ t: 'openTask', id: order[1] as never });
  await until(() => q('[data-part="phone.task.bar"]') !== null);
  // the list's order changes while the page is open (a filter, a re-sort): what the list publishes is what steps
  shell.listOrder?.publish([order[1] as string, order[0] as string]);
  const [next] = await openMenu();
  next?.click();
  await until(() => String((client.nav.nav().main as { id: string }).id) === order[0]);
  expect(String((client.nav.nav().main as { id: string }).id)).toBe(order[0]);
});
