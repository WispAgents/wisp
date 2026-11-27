// Audit B #4: a plugin's later part (registry.extend) renumbers the default order of every plugin registered after it,
// so the registry hands out new objects for extensions that didn't change. The area that drew one (the sidebar's task
// list) remounted it, twice after each load, and WebKit reported "ResizeObserver loop completed" on every page.
import { expect, test } from 'vitest';
import { onMount } from 'solid-js';
import { createClient, peerId } from '@wisp/client';
import { createToaster } from '@wisp/ui';
import type { ExtensionDef, PluginUiLater, ResolvedExtension } from '../src/contract.ts';
import { createShell, type ShellWindow } from '../src/shell.tsx';
import { nullPlatform } from '../src/platform/index.ts';
import { sameExtension } from '../src/slot.tsx';

const C = (() => null) as unknown as ExtensionDef['component'];
const tick = (ms = 0): Promise<void> => new Promise((r) => setTimeout(r, ms));
async function until(ok: () => boolean, ms = 15_000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await tick(5);
}

test('an_area_keeps_its_extension_mounted_when_an_earlier_plugin_s_later_part_renumbers_it', async () => {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  (client.entities as unknown as { loaded: () => () => boolean }).loaded = () => () => true;
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  let mounts = 0;
  const List = (() => {
    onMount(() => (mounts += 1));
    return <div data-testid="list" />;
  }) as unknown as ExtensionDef['component'];
  const later = (): Promise<{ default: PluginUiLater }> =>
    Promise.resolve({ default: { extensions: [{ id: 'early.more', attachTo: { id: 'shell.row:menu' }, component: C }] } });
  // registered first, with a later part: its extensions shift the default order of everything after it
  shell.register({ id: 'wisp.early', extensions: [{ id: 'early.one', attachTo: { id: 'shell.row:menu' }, component: C }], later });
  shell.register({ id: 'wisp.list', extensions: [{ id: 'list.list', attachTo: { id: 'shell.sidebar:list' }, component: List }] });
  const before = shell.context.extensions('shell.sidebar:list')()[0];
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  await until(() => mounts > 0);
  expect(mounts).toBe(1);
  await until(() => shell.context.extensions('shell.row:menu')().some((e) => e.id === 'early.more'));
  const after = shell.context.extensions('shell.sidebar:list')()[0];
  // the registry did hand out a new object (its order moved): that's what used to remount the list
  expect(after).not.toBe(before);
  expect(after?.order).not.toBe(before?.order);
  await tick(20);
  expect(mounts, 'the list mounted once, not again when the later part joined').toBe(1);
  stop();
  shell.dispose();
});

test('the_same_extension_is_its_code_plugin_and_config_not_its_order', () => {
  const def = { id: 'a.b', attachTo: { id: 'x' }, component: C } as ExtensionDef;
  const r = (o: Partial<ResolvedExtension>): ResolvedExtension => ({ id: 'a.b', plugin: 'a', def, config: {}, order: 10, ...o });
  expect(sameExtension(r({}), r({ order: 40 }))).toBe(true);
  expect(sameExtension(r({}), r({ config: { n: 1 } }))).toBe(false);
  expect(sameExtension(r({}), r({ plugin: 'b' }))).toBe(false);
  expect(sameExtension(r({}), r({ def: { ...def } }))).toBe(false);
  expect(sameExtension(undefined, r({}))).toBe(false);
  expect(sameExtension(undefined, undefined)).toBe(true);
});
