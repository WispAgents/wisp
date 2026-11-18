// @vitest-environment happy-dom
// QA desktop r3 #1: a task that ran on Opus said "Claude Code · Default model" in its details and "Claude Code ·
// Default" on its reply box's harness button, so a follow-up looked like it would run on the default model. A
// follow-up runs on the task's own model unless you pick another (runtime run.rs apply_opts keeps it), and the screen
// says so: the task's model in its details, and on the reply box's harness until you pick another there.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type EntityKey } from '@wisp/client';
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

test('a_task_on_opus_says_opus_in_its_details_and_on_its_reply_harness_not_default_qa_desktop_r3_1', async () => {
  const mock = createMockPeer({ epoch: 'e1' });
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
  const task = (): { id: string; summary: { model?: string } } | undefined =>
    client.entities.list<{ id: string; title: string; agent: string; summary: { model?: string } }>('session')().find((r) => r.agent === 'claude' && !!r.summary.model);
  await until(() => task() !== undefined);
  const model = task()?.summary.model ?? '';
  expect(model, 'the mock’s task ran on a model of its own').toMatch(/opus/);
  client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
  const details = (): string => root.querySelector('[data-part="split.details"] [data-kit="ml-name"]')?.textContent ?? '';
  const harness = (): HTMLElement | null => root.querySelector('[data-part="composer.harness"]');
  await until(() => details() !== '' && harness() !== null);
  // its details: the harness and the model it ran on, in the picker's words (QA r15 #3: never the id)
  expect(details()).toBe('Claude Code · Opus 5.5');
  // the reply box: a follow-up runs on the task's own model, and its button says so
  expect(harness()?.getAttribute('title') ?? '').toContain('Claude Code · Opus 5.5');
  expect(harness()?.getAttribute('title') ?? '').not.toContain('Default');
});

// QA r10 desktop #2: a task on Claude Code's default said "Claude Code · Default model" in its facts while its reply
// chip said "Opus": one name everywhere, the same answer (runtime.defaultModel, else the harnesses topic)
test('a_task_on_the_default_model_names_it_in_its_facts_as_its_chip_does', async () => {
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
  const task = (): { id: string } | undefined =>
    client.entities.list<{ id: string; agent: string; summary: { model?: string } }>('session')().find((r) => r.agent === 'claude' && !r.summary.model);
  await until(() => task() !== undefined);
  expect(task(), 'a mock task on Claude Code’s default').toBeDefined();
  client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
  const details = (): string => root.querySelector('[data-part="split.details"] [data-kit="ml-name"]')?.textContent ?? '';
  await until(() => details().includes('Opus'));
  // the harness topic's answer for Claude Code's Default (the seed: Opus), as the reply chip names it
  expect(details()).toBe('Claude Code · Opus');
});


// QA r11 desktop #1: the reply box's Send said "Launch (Enter · Shift+Enter for newline)", scheduled or not. A reply
// sends (v1 composer.js: "Send (Enter · …)"), and with Start later set it says when: "Send at 14:41", as New task's
// Send says "Start at 14:41"
test('the_reply_boxs_send_says_send_and_when_a_follow_up_is_scheduled', async () => {
  const mock = createMockPeer({ epoch: 'e1' });
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
  const task = (): { id: string } | undefined => client.entities.list<{ id: string; state: string }>('session')().find((r) => r.state === 'review');
  await until(() => task() !== undefined);
  client.nav.navigate({ t: 'openTask', id: task()?.id as EntityKey as never });
  const send = (): HTMLElement | null => root.querySelector('[data-part="composer.send"]');
  await until(() => send() !== null && root.querySelector('[data-part="composer.plus"]') !== null);
  expect(send()?.getAttribute('aria-label')).toBe('Send (Enter · Shift+Enter for newline)');
  // + › Start later › In 10 min
  root.querySelector<HTMLElement>('[data-part="composer.plus"]')?.click();
  const item = (re: RegExp): HTMLElement | undefined => [...document.querySelectorAll<HTMLElement>('[role^="menuitem"]')].find((e) => re.test((e.textContent ?? '').trim()));
  await until(() => item(/^Start later/) !== undefined);
  item(/^Start later/)?.click();
  await until(() => item(/^In 10 min/) !== undefined);
  item(/^In 10 min/)?.click();
  await until(() => /^Send at /.test(send()?.getAttribute('aria-label') ?? ''));
  // the time, or the weekday and the time when it falls on another day (after midnight: "Send at Thu 00:09")
  expect(send()?.getAttribute('aria-label')).toMatch(/^Send at (\w{3} )?\d\d:\d\d$/);
  expect(send()?.getAttribute('title')).toMatch(/^Send at (\w{3} )?\d\d:\d\d$/);
});

// QA r14 phone #6: on a 360 px phone the Rerun dialog's bar (the mode, the harness, Rerun) didn't fit one line, so the
// mode's select shrank to its frame and showed no value. The bar is a Wrap group (design/layout.md: only Wrap groups
// wrap): what doesn't fit goes to the next line whole, and the select keeps its words
test('the_rerun_bar_wraps_so_its_pickers_keep_their_values', async () => {
  const mock = createMockPeer({ epoch: 'e1' });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 360 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'android', phone: true, mobile: true }), home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  await until(() => client.entities.keys('session')().length > 0);
  const key = client.entities.keys('session')()[0] as EntityKey;
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.rerun' as never, params: { id: key } } });
  const bar = (): HTMLElement | null => document.querySelector('[data-part~="rerun.bar"]');
  await until(() => bar() !== null);
  expect(bar()?.getAttribute('data-kit'), 'a Wrap group').toBe('wrap');
  // the mode's select says its value
  expect(bar()?.querySelector('[data-kit="select-trigger"]')?.textContent).toContain('Full auto');
});

// QA r15 #3: Save as recipe… said "Runs on claude · opus · high": the run in the picker's words, the harness's name,
// the model's, the thinking level's; on its Default, what Default runs
for (const [given, says] of [
  [{ agent: 'claude', model: 'claude-opus-5-5', effort: 'high' }, 'Runs on Claude Code · Opus 5.5 · High'],
  [{ agent: 'claude', model: '', effort: '' }, 'Runs on Claude Code · Opus'],
] as const) {
  test(`save_as_recipe_names_its_run_in_words_${given.model || 'default'}`, async () => {
    const mock = createMockPeer({ epoch: 'e1', harnesses: true });
    const home = peerId('local');
    const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
    const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
    for (const { def } of pluginUis()) if (def && ['wisp.claude', 'wisp.automations'].includes(def.id)) shell.register(def);
    await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
    const root = document.createElement('div');
    document.body.append(root);
    const stop = shell.render(root);
    stops.push(() => {
      stop();
      shell.dispose();
    });
    client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'automations.recipe' as never, params: { name: 'x', prompt: 'y', project: '', mode: '', ...given } } });
    const line = (): string => [...document.querySelectorAll('[data-part~="automations.recipe"] [data-kit="text"], [role="dialog"] [data-kit="text"]')].map((e) => e.textContent ?? '').find((x) => x.startsWith('Runs on')) ?? '';
    await until(() => line() === says);
    expect(line()).toBe(says);
  });
}
