// @vitest-environment happy-dom
// docs/ux/2026-11-18-phone.md, fewer options showing on the phone apps, through the real client and shell against the
// mock peer: the task page under no tab bar (#2), the reply bar's options behind one chip and its sheet (#1), the side
// panes as a sheet over the task with status pills (#3), New task's one Start (#5), the Tasks bar's Select in its menu
// (#11), an older answer's icons behind ⋯ (#8). Each on Android and iOS; a phone's browser keeps v1's look.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
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

/** `env`: the seed's first task changed files (seed.ts withEnv), so its page has Changes +n −n */
async function start(platform: Platform, o: { env?: boolean } = {}) {
  const mock = createMockPeer({ epoch: 'e1', ...(o.env && { env: true }) });
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
  return { client, root };
}

const phone = (kind: 'android' | 'ios'): Platform => nullPlatform({ kind, phone: true, mobile: true });
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const frame = (root: HTMLElement): HTMLElement | null => root.querySelector<HTMLElement>('[data-kit="frame"]');

async function openTask(client: Awaited<ReturnType<typeof start>>['client'], root: HTMLElement, ...ready: string[]): Promise<string> {
  const task = (): { id: string } | undefined => client.entities.list<{ id: string; title: string }>('session')().find((r) => r.title.startsWith('Add rate limiting'));
  await until(() => task() !== undefined);
  const id = task()?.id as string;
  client.nav.navigate({ t: 'openTask', id: id as EntityKey as never });
  await until(() => ready.every((sel) => root.querySelector(sel) !== null));
  return id;
}

for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_a_task_is_a_pushed_page_the_destinations_give_way_and_come_back_on_the_list`, async () => {
    const { client, root } = await start(phone(kind));
    await until(() => (frame(root)?.querySelector('[data-kit="frame-nav"]') ?? null) !== null);
    expect(frame(root)?.hasAttribute('data-pushed'), 'the list is a destination’s root').toBe(false);
    await openTask(client, root, '[data-part="phone.task.bar"]');
    expect(frame(root)?.hasAttribute('data-pushed'), 'a task hides the tab bar (HIG hidesBottomBarWhenPushed)').toBe(true);
    client.nav.navigate({ t: 'back' });
    await until(() => frame(root)?.getAttribute('data-page') === 'list');
    expect(frame(root)?.hasAttribute('data-pushed')).toBe(false);
  });

  test(`on_${kind}_the_reply_bar_is_attach_one_options_chip_and_send_its_sheet_holds_every_tool`, async () => {
    const { client, root } = await start(phone(kind));
    await openTask(client, root, '[data-pane="main"] [data-kit="reply-box"] [data-kit="c-opts"]');
    const bar = root.querySelector('[data-pane="main"] [data-kit="reply-box"] [data-kit="c-bar"]') as HTMLElement;
    // the bar: attach (or +), the chip, Send; the mode, Delegate, Goal and Send later are not on it
    expect(bar.querySelector('[data-part="composer.send"]'), 'Send').not.toBeNull();
    expect(bar.querySelector('[data-part="composer.attach"], [data-part="composer.plus"]'), 'attach').not.toBeNull();
    for (const part of ['composer.mode', 'composer.team', 'composer.goal', 'composer.sendLater']) expect(bar.querySelector(`[data-part="${part}"]`), `${part} folded`).toBeNull();
    const chip = bar.querySelector<HTMLButtonElement>('[data-kit="c-opts"]');
    expect(chip?.getAttribute('aria-haspopup')).toBe('dialog');
    expect(chip?.textContent, 'it says how the reply runs').toMatch(/auto|Plan|Edits/i);
    chip?.click();
    await until(() => q('[data-part~="composer.options"] [data-kit="opts-list"]') !== null);
    const sheet = q('[data-part~="composer.options"]') as HTMLElement;
    // the permissions as choices, the one in use checked; the agent; the tools the bar held
    const modes = [...sheet.querySelectorAll('[data-part="composer.options.mode"]')];
    expect(modes.length, 'Plan, Edits, Full auto').toBeGreaterThanOrEqual(3);
    expect(modes.filter((m) => m.getAttribute('aria-checked') === 'true')).toHaveLength(1);
    expect(sheet.querySelector('[data-part="composer.options.agent"]'), 'the agent').not.toBeNull();
    if (!bar.querySelector('[data-part="composer.plus"]'))
      for (const part of ['composer.team', 'composer.goal', 'composer.sendLater']) expect(sheet.querySelector(`[data-part="${part}"]`), `${part} in the sheet`).not.toBeNull();
  });

  test(`on_${kind}_the_side_panes_are_a_sheet_over_the_task_and_its_pills_open_them`, async () => {
    const { client, root } = await start(phone(kind), { env: true });
    await until(() => client.entities.list('session')().length > 0);
    // a task of the mock's that changed files (seed.ts withAsk / withEnv): Changes +n −n under its bar
    let id = '';
    for (const row of client.entities.list<{ id: string }>('session')().slice(0, 8)) {
      client.nav.navigate({ t: 'openTask', id: row.id as EntityKey as never });
      await until(() => root.querySelector('[data-part="phone.task.status.changes"]') !== null, 2000);
      if (root.querySelector('[data-part="phone.task.status.changes"]')) {
        id = row.id;
        break;
      }
    }
    const pill = root.querySelector<HTMLButtonElement>('[data-part="phone.task.status.changes"]');
    expect(pill?.textContent).toMatch(/file/);
    pill?.click();
    await until(() => client.nav.nav().side.kind === 'changes' && q('[data-part~="phone.side.sheet"]') !== null);
    // a sheet over the task: the task's page stays on screen, the stage doesn't flip to a side page
    expect(frame(root)?.getAttribute('data-page')).toBe('main');
    expect(root.querySelector('[data-pane="side"]'), 'no side page under the sheet').toBeNull();
    expect(client.nav.nav().side).toEqual({ kind: 'changes', id });
    expect(root.querySelector('[data-part="phone.task.status.changes"]')?.getAttribute('aria-pressed')).toBe('true');
    client.nav.navigate({ t: 'closeSide' });
    await until(() => q('[data-part~="phone.side.sheet"]') === null);
    expect(frame(root)?.getAttribute('data-page')).toBe('main');
  }, 30_000);

  test(`on_${kind}_new_task_has_one_start_and_the_tasks_bar_keeps_select_in_its_menu`, async () => {
    const { client, root } = await start(phone(kind));
    await until(() => client.entities.list('session')().length > 0 && root.querySelector('[data-part="phone.tasks.bar"]') !== null);
    // #11: no Select word in the bar; it is in the bar's ⋮ / ⋯ (and a row's long-press)
    const bar = root.querySelector('[data-part="phone.tasks.bar"]') as HTMLElement;
    expect([...bar.querySelectorAll('button')].some((b) => (b.textContent ?? '').trim() === 'Select'), 'Select not a bar button').toBe(false);
    expect(bar.querySelector('[data-part="phone.tasks.bar.menu.trigger"]'), 'the bar’s menu').not.toBeNull();
    // #5: New task's confirm says Start
    (root.querySelector('[data-part~="phone.tasks.new"]') as HTMLElement).click();
    await until(() => root.querySelector('[data-part="phone.new.bar.confirm"]') !== null);
    expect(root.querySelector('[data-part="phone.new.bar.confirm"]')?.textContent).toBe('Start');
  });
}

test('a_phones_browser_keeps_v1s_reply_bar_no_chip_no_pills_no_pushed_frame', async () => {
  const { client, root } = await start(nullPlatform({ mobile: true }));
  await openTask(client, root, '[data-kit="reply-box"] [data-part="composer.send"]');
  expect(root.querySelector('[data-kit="c-opts"]')).toBeNull();
  expect(root.querySelector('[data-kit="page-status"]')).toBeNull();
  expect(root.querySelector('[data-kit="reply-box"] [data-part="composer.send"]'), 'the browser’s Send').not.toBeNull();
  expect(root.querySelector('[data-kit="tb-more"]'), 'no ⋯ on a turn').toBeNull();
});
