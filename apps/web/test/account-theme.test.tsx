// @vitest-environment happy-dom
// Audit A F6: after picking Light, the account menu's theme item still said "Light theme" (it flips to the other
// theme): the menu read the theme's signal outside the entity store's tracking, so its words never followed. It says
// the theme it switches to, whatever set the current one.
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

test('the_account_menus_theme_item_names_the_other_theme_after_a_pick_audit_a_f6', async () => {
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
  const themeItem = async (): Promise<string> => {
    (root.querySelector('[data-part~="topbar.account"]')?.closest('button') as HTMLElement).click();
    await until(() => document.querySelector('[data-kit="menu-item"][data-value="theme"]') !== null);
    const label = document.querySelector('[data-kit="menu-item"][data-value="theme"] [data-kit="menu-item-label"]')?.textContent ?? '';
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    return label;
  };
  await until(() => root.querySelector('[data-part~="topbar.account"]') !== null);
  for (const [pick, says] of [['light', 'Dark theme'], ['dark', 'Light theme'], ['light', 'Dark theme']] as const) {
    await shell.context.theme.set(pick);
    await until(() => shell.context.theme.resolved() === pick);
    expect(await themeItem(), `with ${pick}`).toBe(says);
  }
});
