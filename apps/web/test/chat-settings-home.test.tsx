// @vitest-environment happy-dom
// Audit ui-r1 doc25b: Settings → Chat (Chat mode, Send shortcut, Chat width, …) are this user's preferences, written
// on this computer. A task on another computer read them from there instead, so it ignored what was picked here.
import { afterEach, expect, test } from 'vitest';
import { createClient, defineCommand, entityKey, peerId, type EntityKey, type PeerId } from '@wisp/client';
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

// Settings' own write (wisp.settings `settings.set`), as the Chat pane sends it
const setSetting = defineCommand<{ peer: string; key: string; value: unknown }>({
  id: 'settings.set',
  owner: (a) => a.peer,
  optimistic: (_b, a) => [{ e: 'setting', id: entityKey(a.peer as PeerId, a.key), op: 'upsert', v: { key: a.key, value: a.value, locked_by: null } as never }],
});

test('a_task_on_another_computer_reads_chat_settings_from_this_one_doc25b', async () => {
  const home = peerId('local');
  const studio = peerId('studio');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform({ kind: 'browser' }), home, window: window as unknown as ShellWindow });
  await client.start([
    { peer: home, name: 'mock', transport: memTransport(createMockPeer({ epoch: 'e1', peer: 'local' })) },
    { peer: studio, name: 'studio', transport: memTransport(createMockPeer({ epoch: 'e2', peer: 'studio' })) },
  ]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => (stop(), shell.dispose()));
  // Chat width picked here; the other computer has a row of its own that isn't this user's pick
  await client.commands.run(setSetting, { peer: home, key: 'settings.chatWidth', value: 'full' });
  await client.commands.run(setSetting, { peer: studio, key: 'settings.chatWidth', value: 'wide' });
  const remote = (): EntityKey | undefined => client.entities.keys('session')().find((k) => k.startsWith(`${studio}:`));
  await until(() => remote() !== undefined);
  client.nav.navigate({ t: 'openTask', id: remote() as never });
  const head = (): HTMLElement | null => root.querySelector('[data-part="split.head"]');
  await until(() => head() !== null);
  expect(head()?.getAttribute('data-width')).toBe('full');
});
