// @vitest-environment happy-dom
// Audit A F26: Projects (place chip → Other folder…) opened with nothing focused (typing went nowhere), and its empty
// state said "Add a folder above, or pick one below" with nothing below. The path field takes the caret, and the
// empty state points only at what's there.
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

test('projects_opens_on_its_path_field_and_its_empty_state_points_at_what_is_there_audit_a_f26', async () => {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.projects' as never, params: null } });
  await until(() => document.querySelector('[data-part~="projects.add"] input') !== null);
  const field = document.querySelector('[data-part~="projects.add"] input');
  await until(() => document.activeElement === field);
  expect(document.activeElement, 'typing goes into the path').toBe(field);
  const list = document.querySelector('[data-part~="projects.list"]')?.textContent ?? '';
  const below = document.querySelector('[data-part~="projects.found"], [data-part~="projects.github"]') !== null;
  if (!below) expect(list).not.toMatch(/below/);
});
