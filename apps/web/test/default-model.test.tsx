// @vitest-environment happy-dom
// UX 2026-11-18 #9: New task's harness chip names the model its Default runs ("Claude Code · Opus") when the harness can
// tell from its own config (the claude plugin reads ~/.claude/settings.json; the mock seeds it), else it says Default.
import { afterEach, expect, test } from 'vitest';
import { createEffect, createRoot } from 'solid-js';
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

// QA r7 desktop #1: with Compare on, the compared agent's chip names its Default's model the same way as the lead's
test('a_compared_agents_chip_names_its_default_model_too', async () => {
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
  await until(() => (root.querySelector('[data-part~="launcher.harness"]')?.textContent ?? '').includes('Opus'));
  // + › Compare with another agent…: the next installed harness (here the mock's one, Claude Code again, from its
  // Default like the lead)
  root.querySelector<HTMLElement>('[data-part~="launcher.plus"]')?.click();
  const item = (): HTMLElement | undefined => [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((e) => /Compare/.test(e.textContent ?? ''));
  await until(() => item() !== undefined);
  item()?.click();
  const run = (): string => root.querySelector('[data-part="launcher.compare.runner"]')?.textContent ?? '';
  await until(() => run() !== '');
  await until(() => run().includes('Opus'));
  expect(run()).toContain('Claude Code');
  expect(run()).toContain('Opus');
  expect(run()).not.toContain('Default');
});

// QA r8 desktop #1: the picker's Default row names the same model the chip does, from the same answer (runtime.defaultModel:
// the project's pin, else the plan's), not only the harnesses topic's config read
test('the_pickers_default_row_names_what_the_chip_names', async () => {
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
  const pill = (): HTMLElement | null => root.querySelector<HTMLElement>('[data-part~="launcher.harness"]');
  // the chip: the project's answer (Sonnet), where the topic says this computer's (Opus)
  await until(() => (pill()?.textContent ?? '').includes('Sonnet'));
  pill()?.click();
  const row = (): string => [...document.querySelectorAll<HTMLElement>('[role="option"], [role="menuitemradio"], [data-part~="runner.picker.model"]')].map((e) => e.textContent ?? '').find((x) => /\(default\)|^Default/.test(x)) ?? '';
  await until(() => row() !== '');
  await until(() => row().includes('Sonnet'));
  expect(row()).toContain('Sonnet');
  expect(row()).not.toContain('Opus');
});

// ShellContext.defaultModel (Compare, the cards, the list's tip): the folder's answer, else the computer's harnesses
// topic, which a Solid reader follows as its rows land (the store mirrors the client's rows)
test('the_shells_default_model_follows_the_harnesses_topic_as_it_lands', async () => {
  const mock = createMockPeer({ epoch: 'e1', harnesses: true });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  const words: (string | undefined)[] = [];
  // read before anything has landed, in a Solid effect: it must run again when the topic's rows come
  const dispose = createRoot((d) => {
    createEffect(() => words.push(shell.context.defaultModel?.('claude', { peer: home })));
    return d;
  });
  stops.push(() => {
    dispose();
    shell.dispose();
  });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  client.peers.watch(home, 'plugin:wisp.runtime:harnesses', client.scope.child('t'), { passive: true });
  await until(() => words.at(-1) === 'Opus');
  expect(words[0]).toBeUndefined();
  expect(words.at(-1)).toBe('Opus');
});
