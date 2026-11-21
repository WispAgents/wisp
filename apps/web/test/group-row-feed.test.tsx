// @vitest-environment happy-dom
// audit C tasks #5: a compare group's row put vs Compare and the time on the title's line, so the title kept a few
// letters ("TOPI… · Pick one" on a 360 px phone, 97 of 318 px on a desktop). It's laid out as a task row: the title
// has the first line, vs Compare and the time sit under it.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
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
  document.body.innerHTML = '';
});

test('a_compare_groups_row_gives_its_title_the_first_line_as_a_task_row_does_audit_c_5', async () => {
  const mock = createMockPeer({ epoch: 'e1', trial: true });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def) shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  const group = (): HTMLElement | null => root.querySelector('[data-kit="list-row"][data-part~="list.group"]');
  await until(() => group() !== null, 8000);
  expect(group()?.getAttribute('data-layout')).toBe('feed');
  expect(group()?.querySelector('[data-part~="list.group.compare"]')?.closest('[data-kit="list-row-pills"]'), 'vs Compare with the pills').not.toBeNull();
});
