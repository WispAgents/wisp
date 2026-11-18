// @vitest-environment happy-dom
// The one project selector everywhere a project is chosen (owner, 2026-11-15 20:05; docs/v2-plan.md §3): New task's
// place is the sidebar's selector (no place chips), it opens the same ⌘P list on the box's place (click = only that
// project, + adds one, No folder and Other folder… in the list), and the phone apps' Tasks screen has it in its bar in
// place of the chip row. Through the real client and shell against the mock peer.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
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

async function start(platform: Platform, width: number) {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width });
  const shell = createShell({
    client,
    toaster: createToaster(),
    platform,
    home,
    window: window as unknown as ShellWindow,
  });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  // three projects (the mock adds them as `wisp add` would)
  const apply = {
    id: 'resources.apply',
    owner: () => home,
    optimistic: 'none' as const,
  };
  const resources = ['api', 'web', 'docs'].map((name) => ({
    kind: 'project',
    name,
    spec: { path: `/code/${name}` },
  }));
  await client.commands.run(apply as never, { resources } as never);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, shell, root };
}

const q = <T extends HTMLElement = HTMLElement>(sel: string): T | null => document.querySelector<T>(sel);
const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-part="shell.palette.row"]')];
const row = (label: string): HTMLElement | undefined => rows().find((r) => r.querySelector('[data-kit="pal-label"]')?.textContent === label);
const place = (): HTMLElement | null => q('[data-part="launcher.place"]');
const placeName = (): string => place()?.querySelector('[data-kit="p-name"]')?.textContent ?? '';

/** the place list, open with every row drawn */
async function openPlaces(): Promise<void> {
  place()?.click();
  await until(() => ['api', 'docs', 'web', 'No folder', 'Other folder…'].every((l) => row(l) !== undefined));
}

async function home(width = 1440) {
  const s = await start(nullPlatform(), width);
  s.client.nav.navigate({ t: 'home' });
  await until(() => place() !== null);
  await until(() => placeName() !== '' && placeName() !== 'No folder');
  return s;
}

test('the_launcher_uses_the_sidebars_one_project_selector_not_v1s_place_chips', async () => {
  await home();
  const sel = place();
  expect(sel?.getAttribute('data-kit')).toBe('p-select');
  // the same component the sidebar's head draws
  expect(q('[data-part="topbar.project-select"]')?.getAttribute('data-kit')).toBe('p-select');
  // inside the place line, no chips: no project chips, no No folder chip, no Other folder… chip
  const line = q('[data-part="launcher.projects"]');
  expect(line?.querySelectorAll('[data-kit="lp-chip"]').length).toBe(0);
  expect(line?.textContent).not.toContain('No folder');
  expect(placeName()).toBe('api');
  expect(sel?.querySelectorAll('[data-kit="p-dot"]').length).toBe(1);
});

test('the_launchers_selector_opens_the_switcher_list_with_search_dots_and_no_folder_and_other_folder', async () => {
  await home();
  await openPlaces();
  const labels = rows().map((r) => r.querySelector('[data-kit="pal-label"]')?.textContent);
  expect(labels).toEqual(['api', 'docs', 'web', 'No folder', 'Other folder…']);
  // each project leads with its colour's dot and has its + (add it to the task)
  for (const name of ['api', 'docs', 'web']) expect(row(name)?.querySelector('[data-part="shell.palette.add"]'), name).not.toBeNull();
  // the box's own words, not the list's switcher
  expect(q<HTMLInputElement>('[data-part="shell.palette.input"] input')?.placeholder).toBe('Pick a project…');
});

test('click_on_a_project_in_the_launchers_list_picks_only_that_project', async () => {
  await home();
  await openPlaces();
  row('web')?.querySelector<HTMLElement>('[data-part="shell.palette.add"]')?.click(); // api + web
  await until(() => placeName() === 'api + web');
  row('docs')?.click(); // only docs, whatever was picked
  await until(() => row('api') === undefined && placeName() === 'docs');
  expect(row('api'), 'the list closed').toBeUndefined();
  expect(placeName()).toBe('docs');
  expect(place()?.hasAttribute('data-several')).toBe(false);
});

test('plus_on_a_row_adds_that_project_to_the_task_and_the_list_stays_open', async () => {
  await home();
  await openPlaces();
  row('web')?.querySelector<HTMLElement>('[data-part="shell.palette.add"]')?.click();
  await until(() => placeName() === 'api + web' && (row('web')?.querySelector('[data-part="shell.palette.check"]') ?? null) !== null);
  // still open, web now checked
  expect(row('web')?.querySelector('[data-part="shell.palette.check"]')).not.toBeNull();
  expect(placeName()).toBe('api + web');
  expect(place()?.hasAttribute('data-several')).toBe(true);
  expect(place()?.querySelectorAll('[data-kit="p-dot"]').length).toBe(2);
});

test('no_folder_and_other_folder_are_reachable_inside_the_selectors_list', async () => {
  const { client } = await home();
  await openPlaces();
  row('No folder')?.click();
  await until(() => placeName() === 'No folder');
  expect(placeName()).toBe('No folder');
  expect(place()?.querySelectorAll('[data-kit="p-dot"]').length).toBe(0);
  await openPlaces();
  row('Other folder…')?.click();
  await until(() => client.nav.nav().overlays.some((o) => String(o.id) === 'shell.projects'));
  // Other folder… is the Add a folder dialog (a folder that isn't a project yet)
  expect(client.nav.nav().overlays.some((o) => String(o.id) === 'shell.projects')).toBe(true);
});

// Android with one computer: the one selector is the M3 top app bar's title menu (QA round 3 A10), the same ⌘P list
test('the_phone_tasks_screen_on_android_has_the_one_selector_as_its_title_menu_and_no_project_chips', async () => {
  const { root } = await start(nullPlatform({ kind: 'android', phone: true, mobile: true }), 390);
  const title = (): HTMLElement | null => root.querySelector<HTMLElement>('[data-part~="phone.tasks.bar"] [data-kit="app-bar-title-button"]');
  await until(() => title() !== null);
  expect(title()?.textContent).toContain('All projects');
  expect(root.querySelector('[data-part~="phone.tasks.projects"]'), 'no chip row').toBeNull();
  // a tap opens the same ⌘P list
  title()?.click();
  await until(() => row('api') !== undefined);
  expect(row('All projects')).toBeDefined();
});

for (const kind of ['ios'] as const) {
  test(`the_phone_tasks_screen_on_${kind}_has_the_one_selector_in_its_bar_and_no_project_chips`, async () => {
    const { root } = await start(nullPlatform({ kind, phone: true, mobile: true }), 390);
    await until(() => (root.querySelector('[data-part="phone.tasks.project"]')?.closest('[data-part~="phone.tasks.bar"]') ?? null) !== null);
    const sel = root.querySelector('[data-part="phone.tasks.project"]');
    expect(sel?.getAttribute('data-kit')).toBe('p-select');
    expect(sel?.getAttribute('data-where')).toBe('bar');
    expect(sel?.closest('[data-part~="phone.tasks.bar"]'), 'in the top bar').not.toBeNull();
    expect(root.querySelector('[data-part~="phone.tasks.projects"]'), 'no chip row').toBeNull();
    // a tap opens the same ⌘P list
    (sel as HTMLElement).click();
    await until(() => row('api') !== undefined);
    expect(row('All projects')).toBeDefined();
  });
}

test('the_place_list_opened_again_with_no_new_task_box_behind_it_closes_rather_than_showing_nothing', async () => {
  const { client } = await start(nullPlatform(), 1440);
  // a reload or Back to the URL of an open place list: no prompt box set its place
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: { only: ['shell.places'] } } });
  expect(client.nav.nav().overlays.some((o) => String(o.id) === 'shell.palette'), 'opened').toBe(true);
  await until(() => !client.nav.nav().overlays.some((o) => String(o.id) === 'shell.palette'));
  expect(client.nav.nav().overlays.some((o) => String(o.id) === 'shell.palette')).toBe(false);
});
