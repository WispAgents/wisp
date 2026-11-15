// ADR-0065 step 13, the Tasks screen on phones: the platform's top app bar (the computer filter as its title, Search,
// the me menu) and New task as the FAB (Android) / the bar's compose button (iOS) in place of v1's desktop head and New
// task row; a phone's browser keeps v1's head. The computer filter narrows the merged list to one computer's rows.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createToaster } from '@wisp/ui';
import { createShell, type ShellWindow } from '../src/shell.tsx';
import { nullPlatform } from '../src/platform/index.ts';
import type { Platform } from '../src/platform/types.ts';
import { ALL_COMPUTERS, computerOptions, computerTitle, inComputer, knownComputer } from '../src/frame/computer-filter.ts';

const tick = (ms = 0): Promise<void> => new Promise((r) => setTimeout(r, ms));

async function mountOn(platform: Platform): Promise<{ root: HTMLElement; stop: () => void }> {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  const root = document.createElement('div');
  document.body.append(root);
  const stopRender = shell.render(root);
  // the phone frame, the head's chunk and the FAB load lazily: wait for the bar itself (the phone's app bar, or the
  // browser's v1 head), with a deadline a loaded CI machine meets (a fixed 400 ms failed on macOS CI, 51bbc0aa)
  const ready = (): boolean => !!root.querySelector('[data-kit="frame-top"] > [data-kit="app-bar"], [data-kit="topbar"]');
  for (const until = Date.now() + 5000; !ready() && Date.now() < until; ) await tick(10);
  await tick(30);
  return {
    root,
    stop: () => {
      stopRender();
      shell.dispose();
      root.remove();
    },
  };
}

afterEach(() => document.documentElement.removeAttribute('data-platform'));

for (const kind of ['android', 'ios'] as const) {
  test(`the_tasks_screen_on_${kind}_is_the_platform_bar_with_the_computer_filter_and_new_task_not_v1s_head`, async () => {
    const { root, stop } = await mountOn(nullPlatform({ kind, phone: true, mobile: true }));
    const bar = root.querySelector('[data-kit="frame-top"] > [data-kit="app-bar"]');
    expect(bar, 'the root TopAppBar is the frame’s top').not.toBeNull();
    expect(bar?.getAttribute('data-kind')).toBe('root');
    expect(bar?.getAttribute('data-part')).toBe('phone.tasks.bar');
    // the computer filter is the title button: All until one is picked
    expect(bar?.querySelector('[data-kit="app-bar-title-button"]')?.textContent).toContain('All');
    expect(bar?.querySelector('[aria-label="Search"]'), 'Search in the bar, no keyboard hint').not.toBeNull();
    expect(root.querySelector('[data-part="phone.tasks.me-trigger"], [data-part="phone.tasks.me.trigger"]'), 'the me menu').not.toBeNull();
    // v1's desktop head and New task row are gone on the phone
    expect(root.querySelector('[data-kit="topbar"]')).toBeNull();
    expect(root.querySelector('[data-part="sidebar.new"], [data-kit="split-top"]')).toBeNull();
    if (kind === 'android') expect(root.querySelector('[data-pane="list"] [data-kit="fab"][data-part="phone.tasks.new"]'), 'the extended FAB over the list').not.toBeNull();
    else {
      expect(root.querySelector('[data-kit="fab"]'), 'iOS draws no FAB').toBeNull();
      expect(bar?.querySelector('[data-part="phone.tasks.new"]'), 'New task is the bar’s compose button').not.toBeNull();
    }
    stop();
  });
}

test('a_phones_browser_keeps_v1s_head_and_new_task_row_and_draws_no_app_bar_or_fab', async () => {
  const { root, stop } = await mountOn(nullPlatform({ mobile: true }));
  expect(document.documentElement.hasAttribute('data-platform')).toBe(false);
  expect(root.querySelector('[data-kit="topbar"]'), 'v1’s head').not.toBeNull();
  expect(root.querySelector('[data-kit="app-bar"]')).toBeNull();
  expect(root.querySelector('[data-kit="fab"][data-part="phone.tasks.new"]')).toBeNull();
  stop();
});

test('the_computer_filter_is_all_or_a_saved_computer_and_one_gone_reads_as_all', () => {
  const peers = ['mac', 'studio'];
  const name = (p: string): string => ({ mac: 'Mac mini', studio: 'Studio' })[p] ?? '';
  expect(computerTitle(ALL_COMPUTERS, peers, name, 'All')).toBe('All');
  expect(computerTitle('studio', peers, name, 'All')).toBe('Studio');
  expect(computerTitle('gone', peers, name, 'All'), 'forgotten: All').toBe('All');
  expect(knownComputer(undefined, peers)).toBe(ALL_COMPUTERS);
  expect(computerOptions('mac', peers, name, 'All')).toEqual([
    { id: 'all', label: 'All', on: false },
    { id: 'mac', label: 'Mac mini', on: true },
    { id: 'studio', label: 'Studio', on: false },
  ]);
  expect(inComputer('all', 'mac:t1')).toBe(true);
  expect(inComputer('mac', 'mac:t1')).toBe(true);
  expect(inComputer('mac', 'mac-2:t1'), 'a peer id that prefixes another is not it').toBe(false);
  expect(inComputer('studio', 'mac:t1')).toBe(false);
});
