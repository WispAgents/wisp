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
  const where = (): HTMLElement | null => root.querySelector<HTMLElement>('[data-part~="launcher.where"]');
  await until(() => where()?.textContent?.includes('·') === true);
  where()?.click();
  const row = (v: string): HTMLElement | undefined =>
    [...document.querySelectorAll<HTMLElement>('[data-kit="menu"][data-state="open"] [data-kit="menu-item"]')].find((e) => e.getAttribute('data-value') === v);
  await until(() => row('how:inPlace') !== undefined);
  const inPlace = row('how:inPlace')?.textContent ?? '';
  expect(inPlace).toContain('In place');
  expect(inPlace, 'the pick is kept').toContain('from now on');
  expect(inPlace, 'and the CLI keeps its own').toContain('The CLI keeps worktrees');
  expect(row('how:worktree')?.textContent ?? '').toContain('from now on');
});
