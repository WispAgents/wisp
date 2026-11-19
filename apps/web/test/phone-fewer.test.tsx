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
import { pluginUis } from '../src/plugins.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

/** `env`: the seed's worktree task changed files (seed.ts withEnv), so its page has Changes +n −n */
/** `plugins`: the plugin UIs to register (the side panes are shell-task's) */
async function start(platform: Platform, o: { env?: boolean; plugins?: readonly string[] } = {}) {
  const mock = createMockPeer({ epoch: 'e1', ...(o.env && { env: true }) });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def && o.plugins?.includes(def.id)) shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, root, mock };
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
    // the setup list's rows say what they are: the place in two words (its sentence stays the tooltip)
    await until(() => root.querySelector('[data-part~="launcher.where"]') !== null);
    expect(root.querySelector('[data-part~="launcher.where"]')?.getAttribute('aria-label')).toBe('This computer');
    for (const part of ['launcher.mode', 'launcher.harness', 'launcher.team', 'launcher.more']) expect(root.querySelector(`[data-part~="${part}"]`), part).not.toBeNull();
    // QA r6 phone #11: the field in a phone's words, not the desktop's "Paste screenshots, drop logs or videos…"
    const field = root.querySelector<HTMLTextAreaElement>('[data-kit="launcher"] textarea');
    expect(field?.placeholder).toBe('What should I go do?');
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

// #14: a gone worktree is said in the Changes pane in words, not as a toast holding its raw path (the browser keeps its
// toast: the same read, not quiet there)
for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_a_gone_folder_is_the_changes_panes_empty_state_in_words_and_no_raw_path_toast`, async () => {
    const { client, root, mock } = await start(phone(kind), { plugins: ['wisp.shell-task'] });
    mock.handle('git.changes', () => {
      throw { code: 'not_found', message: 'no such folder: /Users/me/.wisp/worktrees/wisp-01a1eb6bf0bd' };
    });
    const id = await openTask(client, root, '[data-part="phone.task.bar"]');
    client.nav.navigate({ t: 'openSide', side: { kind: 'changes', id: id as never } });
    await until(() => q('[data-part~="side.changes.refused"]') !== null);
    expect(q('[data-part~="side.changes.refused"]')?.textContent).toContain('This task’s folder is gone');
    expect(q('[data-part~="side.changes.refused"]')?.textContent, 'no path in it').not.toContain('/Users/');
    expect(document.body.textContent, 'no toast with the raw path').not.toContain('no such folder');
  });
}


// #10: a task row on a phone is two lines: the title, then the harness's logo and what the task says (the computer
// only when it isn't this one); no third run line
for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_a_task_row_is_two_lines_with_the_harness_mark_and_no_run_line`, async () => {
    const { client, root } = await start(phone(kind), { plugins: ['wisp.shell-list'] });
    await until(() => client.entities.list('session')().length > 0 && root.querySelector('[data-pane="list"] [data-kit="list-row"][data-row-lines]') !== null);
    const rows = [...root.querySelectorAll<HTMLElement>('[data-pane="list"] [data-kit="list-row"][data-layout="feed"]')];
    expect(rows.length).toBeGreaterThan(0);
    const said = rows.filter((r) => (r.querySelector('[data-kit="list-row-sub"]')?.textContent ?? '').trim() !== '');
    expect(said.length, 'rows with something to say').toBeGreaterThan(0);
    for (const r of said) {
      expect(r.getAttribute('data-row-lines'), r.textContent ?? '').toBe('2');
      expect(r.querySelector('[data-kit="list-row-foot"] [data-kit="row-run"], :scope > [data-kit="row-run"]'), 'no run line under it').toBeNull();
    }
    // a row with nothing said yet: harness · model is its second line, never a third under an empty one
    expect(rows.filter((r) => r.getAttribute('data-row-lines') === '3').map((r) => r.textContent)).toEqual([]);
  });
}

// #14, the Shell: a gone folder is said in the pane, no raw-path toast
test('on_android_a_shell_with_a_gone_folder_says_so_in_the_pane_and_no_raw_path_toast', async () => {
  const { client, root, mock } = await start(phone('android'), { plugins: ['wisp.shell-task'] });
  // the Shell's own reads, and the side panel's background count of Changes
  for (const cmd of ['terminal.shell.info', 'terminal.shell.start', 'git.changes'])
    mock.handle(cmd, () => {
      throw { code: 'not_found', message: 'no such folder: /Users/me/.wisp/worktrees/wisp-01a1eb6bf0bd' };
    });
  const id = await openTask(client, root, '[data-part="phone.task.bar"]');
  client.nav.navigate({ t: 'openSide', side: { kind: 'shell', id: id as never } });
  await until(() => (q('[data-part~="phone.side.sheet"]')?.textContent ?? '').includes('nowhere to start'));
  expect(q('[data-part~="phone.side.sheet"]')?.textContent).toContain('This task’s folder is gone');
  expect(document.body.textContent, 'no toast with the raw path').not.toContain('no such folder');
});

// QA r6 #5: the side sheet's tabs are labelled, every pane one tap away (Preview and Thread too, none behind +)
for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_the_side_sheet_has_every_pane_as_a_labelled_tab_none_behind_more`, async () => {
    const { client, root } = await start(phone(kind), { plugins: ['wisp.shell-task', 'wisp.threads'] });
    const id = await openTask(client, root, '[data-part="phone.task.bar"]');
    client.nav.navigate({ t: 'openSide', side: { kind: 'changes', id: id as never } });
    const labels = (): string[] => [...document.querySelectorAll('[data-part~="phone.side.sheet"] [data-part="panel.tabs"] [data-kit="tab"]')].map((x) => (x.querySelector('[data-kit="tab-label"]')?.textContent ?? '').trim());
    await until(() => labels().includes('Thread'));
    for (const l of ['Changes', 'PR', 'Shell', 'Preview', 'Thread']) expect(labels(), l).toContain(l);
    expect(labels().every((l) => l !== ''), 'no unlabelled icon tab').toBe(true);
    expect(q('[data-part~="phone.side.sheet"] [data-part~="panel.tabs.more"]'), 'nothing behind +').toBeNull();
    // QA r7 #1: a screen reader's way out: the grabber is a button named Close, and pressing it closes the sheet
    const grabber = q('[data-part~="phone.side.sheet"] > [data-kit="sheet-handle"]') as HTMLElement;
    expect(grabber.tagName).toBe('BUTTON');
    expect(grabber.getAttribute('aria-label')).toBe('Close');
    // the phone chunk has bound the sheet (its drag, the grabber's press)
    await until(() => (q('[data-part~="phone.side.sheet"]')?.getAttribute('data-gesture') ?? '').includes('sheet.drag-down'));
    grabber.click();
    await until(() => q('[data-part~="phone.side.sheet"]') === null && client.nav.nav().side.kind !== 'changes');
    expect(client.nav.nav().side.kind).not.toBe('changes');
  });
}

// QA r6 #13 (UX phone #6): the task's ⋮ on Android is an M3 bottom sheet (its rows, drill-in submenus); iOS keeps its menu
test('on_android_the_tasks_menu_is_a_bottom_sheet_of_three_groups_with_drill_ins', async () => {
  const { client, root } = await start(phone('android'));
  await openTask(client, root, '[data-part="phone.task.bar.menu.trigger"]');
  (root.querySelector('[data-part="phone.task.bar.menu.trigger"]') as HTMLElement).click();
  await until(() => q('[data-kit="sheet-menu"]') !== null);
  const rows = [...document.querySelectorAll<HTMLElement>('[data-kit="sheet-menu"] [data-kit="sheet-item"]')];
  const words = rows.map((r) => (r.querySelector('[data-kit="sheet-item-label"]')?.textContent ?? '').trim());
  expect(rows.length, 'nine rows at most (the 27-row menu is folded)').toBeLessThanOrEqual(9);
  for (const w of ['This task', 'Show']) expect(words, `${w} in ${JSON.stringify(words)}`).toContain(w);
  // Keep and copy › (a group of one is left inline by the kit's menuSections: no drill-in holds a single row)
  expect(words.includes('Keep and copy') || words.includes('Copy task ID'), JSON.stringify(words)).toBe(true);
  expect(document.querySelectorAll('[data-kit="sheet-menu"] [role="separator"], [data-kit="sheet-menu"] [data-kit="sheet-separator"]').length, 'three groups').toBe(2);
  // a drill-in: Show › lists what you look at; Back returns
  rows[words.indexOf('Show')]?.click();
  await until(() => q('[data-kit="sheet-menu"] [data-back]') !== null);
  expect(document.querySelector('[data-kit="sheet-menu"]')?.textContent).toMatch(/Transcript|Changes/);
});

test('on_ios_the_tasks_menu_stays_a_pull_down_menu_not_a_sheet', async () => {
  const { client, root } = await start(phone('ios'));
  await openTask(client, root, '[data-part="phone.task.bar"]');
  expect(root.querySelector('[data-part="phone.task.bar"] [data-kit="sheet-menu"]')).toBeNull();
  const trigger = root.querySelector<HTMLElement>('[data-part="phone.task.bar"] [aria-haspopup="menu"]');
  expect(trigger, 'the ⋯').not.toBeNull();
  trigger?.click();
  await until(() => q('[role="menu"]') !== null);
  expect(q('[data-kit="sheet-menu"]'), 'not an M3 sheet on iOS').toBeNull();
});

// QA r14 #3: an overlay view (Share…) brings only its body, no Cancel: on a phone it is never drawn as an alert (a
// confirm's centred lines, no ✕) but as the platform's own surface with its way out (iOS: a sheet, its ✕ and the Close
// grabber; Android: the M3 dialog with its ✕)
for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_share_is_a_${kind === 'ios' ? 'sheet' : 'dialog'}_with_its_close_not_an_alert`, async () => {
    const { client, root } = await start(phone(kind), { plugins: ['wisp.stuff'] });
    const id = await openTask(client, root, '[data-part="phone.task.bar"]');
    client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'stuff.share' as never, params: { session: id } } });
    await until(() => q('[data-part~="share.dialog"]') !== null && (q('[data-part~="share.dialog"]')?.getAttribute('data-gesture') ?? '').includes('sheet') === (kind === 'ios'));
    const box = q('[data-part~="share.dialog"]') as HTMLElement;
    expect(box.hasAttribute('data-alert'), 'not an alert').toBe(false);
    expect(box.getAttribute('data-kit')).toBe(kind === 'ios' ? 'sheet' : 'dialog');
    const close = box.querySelector<HTMLElement>('[data-kit="dialog-head"] > [data-kit="icon-button"]');
    expect(close?.getAttribute('aria-label'), 'its ✕').toBe('Close');
    close?.click();
    await until(() => q('[data-part~="share.dialog"]') === null);
    expect(q('[data-part~="share.dialog"]'), 'closed by its ✕').toBeNull();
  });
}
