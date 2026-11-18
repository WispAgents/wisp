// @vitest-environment happy-dom
// UX 2026-11-18 #9: New task's harness chip names the model its Default runs ("Claude Code · Opus") when the harness can
// tell from its own config (the claude plugin reads ~/.claude/settings.json; the mock seeds it), else it says Default.
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

test('new_tasks_chip_names_the_model_claude_codes_default_runs_ux_2026_11_18_9', async () => {
  const mock = createMockPeer({ epoch: 'e1', harnesses: true });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def?.id === 'wisp.claude') shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  const pill = (): string => root.querySelector('[data-part~="launcher.harness"]')?.textContent ?? '';
  await until(() => pill().includes('Opus'));
  expect(pill()).toContain('Claude Code');
  // the model Default runs, not "Default"
  expect(pill()).toContain('Opus');
  expect(pill()).not.toContain('Default');
});

// owner's delegate 2026-11-18: in a project that pins a model, Default is that model (what Claude Code will run there)
test('in_a_project_that_pins_a_model_the_chip_names_the_projects_model', async () => {
  const mock = createMockPeer({ epoch: 'e1', harnesses: true, computers: true });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def?.id === 'wisp.claude') shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  const pill = (): string => root.querySelector('[data-part~="launcher.harness"]')?.textContent ?? '';
  // the seed's project here is picked (New task starts in it) and pins Sonnet; this computer's default is Opus
  await until(() => pill().includes('Sonnet'));
  expect(pill()).toContain('Sonnet');
  expect(pill()).not.toContain('Opus');
});
