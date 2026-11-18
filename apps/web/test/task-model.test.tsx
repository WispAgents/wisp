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
  // its details: the harness and the model it ran on
  expect(details()).toBe(`Claude Code · ${model}`);
  // the reply box: a follow-up runs on the task's own model, and its button says so
  expect(harness()?.getAttribute('title') ?? '').toContain(`Claude Code · ${model}`);
  expect(harness()?.getAttribute('title') ?? '').not.toContain('Default');
});
