// @vitest-environment happy-dom
// Audit C4: New task's Where › In place writes the shared Worktree per task switch (`worktrees.worktree`), kept for every
// later New task, as v1's launcher toggle was (`defaults.worktree`). It used to say nothing, and five CLI launches after
// it edited the user's checkout. The menu now says the pick is kept, and that the CLI keeps its own default.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createShell, nullPlatform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.body.innerHTML = '';
});

test('where_in_place_says_it_is_kept_for_every_new_task_and_the_cli_keeps_its_own_worktree_audit_c4', async () => {
  const mock = createMockPeer({ epoch: 'e1', computers: true });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => (stop(), shell.dispose()));
  // since 2026-11-27 the selector beside the project's ("on This computer · Worktree"), opening the palette dialog
  const where = (): HTMLElement | null => root.querySelector<HTMLElement>('[data-part~="launcher.runon"]');
  await until(() => where()?.textContent?.includes('·') === true);
  where()?.click();
  const row = (label: string): HTMLElement | undefined =>
    [...document.querySelectorAll<HTMLElement>('[data-part="shell.palette.row"]')].find((r) => r.querySelector('[data-kit="pal-label"]')?.textContent === label);
  await until(() => row('In place') !== undefined);
  const inPlace = row('In place')?.textContent ?? '';
  expect(inPlace, 'the pick is kept').toContain('from now on');
  expect(inPlace, 'and the CLI keeps its own').toContain('The CLI keeps worktrees');
  expect(row('Worktree')?.textContent ?? '').toContain('from now on');
  expect(row('This computer'), 'the computers are in the same list').toBeDefined();
  row('In place')?.click();
  await until(() => where()?.textContent?.includes('In place') === true);
});

test('the_list_head_filters_by_computer_beside_the_project_selector', async () => {
  const home = peerId('local');
  const studio = peerId('studio');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  await client.start([
    { peer: home, name: 'mock', transport: memTransport(createMockPeer({ epoch: 'e1' })) },
    { peer: studio, name: 'studio', transport: memTransport(createMockPeer({ epoch: 'e2' })) },
  ]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => (stop(), shell.dispose()));
  const sel = (): HTMLElement | null => root.querySelector<HTMLElement>('[data-part~="topbar.computer-select"]');
  const name = (): string => sel()?.querySelector('[data-kit="p-name"]')?.textContent ?? '';
  await until(() => name() === 'All computers');
  sel()?.click();
  const row = (label: string): HTMLElement | undefined =>
    [...document.querySelectorAll<HTMLElement>('[data-part="shell.palette.row"]')].find((r) => r.querySelector('[data-kit="pal-label"]')?.textContent === label);
  await until(() => row('studio') !== undefined);
  expect(row('All computers')?.textContent ?? '').toContain('Showing now');
  expect(row('This computer')).toBeDefined();
  row('studio')?.click();
  await until(() => name() === 'studio');
});
