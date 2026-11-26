// ADR-0067, ADR-0105: a plugin UI's `later` part (what only a press reaches: row menu items, the palette's, prompt
// sources) loads once the first screen is drawn (the home list here, two frames painted) and joins the same plugin; and strings a plugin's lazy code adds to its catalogue
// (i18n/lazy.ts) show through the client, both halves of a catalogue with full-id keys included.
import { expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createToaster } from '@wisp/ui';
import type { ExtensionDef, PluginUiDef, PluginUiLater } from '../src/contract.ts';
import { createShell, type ShellWindow } from '../src/shell.tsx';
import { nullPlatform } from '../src/platform/index.ts';

const C = (() => null) as unknown as ExtensionDef['component'];
const tick = (ms = 0): Promise<void> => new Promise((r) => setTimeout(r, ms));
/** until `ok`, to a clock deadline (the first screen is two frames after the home list is here) */
async function until(ok: () => boolean, ms = 2000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await tick(5);
}
/** the home list is here (its snapshot landed): the first screen can be drawn */
const listHere = (client: ReturnType<typeof createClient>): void => {
  (client.entities as unknown as { loaded: () => () => boolean }).loaded = () => () => true;
};

function setup() {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  // the test plugins' items (the shell's own sit there too)
  const menu = (): string[] => shell.context.extensions('shell.row:menu')().filter((e) => e.plugin !== 'shell').map((e) => `${e.plugin}:${e.id}`);
  return { client, shell, menu };
}

const later = (ids: string[]): (() => Promise<{ default: PluginUiLater }>) => () =>
  Promise.resolve({ default: { extensions: ids.map((id) => ({ id, attachTo: { id: 'shell.row:menu' }, component: C })) } });

test('a_plugin_s_later_part_joins_it_once_the_first_screen_draws_after_its_own_extensions', async () => {
  const { client, shell, menu } = setup();
  shell.register({ id: 'wisp.demo', extensions: [{ id: 'demo.first', attachTo: { id: 'shell.row:menu' }, component: C }], later: later(['demo.rename', 'demo.ship']) });
  await tick();
  // not before the first render: the first page's work comes first
  expect(menu()).toEqual(['wisp.demo:demo.first']);
  const root = document.createElement('div');
  document.body.append(root);
  listHere(client);
  const stop = shell.render(root);
  // not as it renders: the first screen is drawn two frames after the home list is here, then the page is idle
  expect(menu()).toEqual(['wisp.demo:demo.first']);
  await until(() => menu().length === 3);
  expect(menu()).toEqual(['wisp.demo:demo.first', 'wisp.demo:demo.rename', 'wisp.demo:demo.ship']);
  expect(performance.getEntriesByName('wisp:first-screen').length, 'the mark the budget gate reads').toBeGreaterThan(0);
  stop();
  shell.dispose();
});

test('a_plugin_registered_after_the_first_screen_loads_its_later_part_at_once_and_one_switched_off_first_gets_none', async () => {
  const { client, shell, menu } = setup();
  listHere(client);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  // the first screen drawn: from now on a later part loads as its plugin registers
  let drawn = false;
  shell.afterFirstScreen(() => (drawn = true));
  await until(() => drawn);
  shell.register({ id: 'wisp.on', later: later(['on.item']) });
  shell.register({ id: 'wisp.off', later: later(['off.item']) });
  shell.unregister('wisp.off');
  await tick();
  expect(menu()).toEqual(['wisp.on:on.item']);
  stop();
  shell.dispose();
});

test('strings_a_plugin_s_lazy_code_adds_to_its_catalogue_show_through_the_client_in_both_halves', () => {
  const { client, shell } = setup();
  const catalogue: Record<string, string> = { 'demo.a': 'A', 'wisp.demo.b': 'B' };
  shell.register({ id: 'wisp.demo', catalogue } as PluginUiDef);
  expect(client.i18n.t('demo.c')).toBe('demo.c');
  // what src/i18n/lazy.ts does when the code that needs them loads
  Object.assign(catalogue, { 'demo.c': 'C', 'wisp.demo.d': 'D' });
  expect([client.i18n.t('demo.a'), client.i18n.t('wisp.demo.b'), client.i18n.t('demo.c'), client.i18n.t('wisp.demo.d')]).toEqual(['A', 'B', 'C', 'D']);
  // each key still has one owner: the namespace's half doesn't answer for a full-id key
  expect(client.i18n.has('wisp.demo.d')).toBe(true);
  shell.dispose();
});

test('laterReady_waits_for_parts_on_their_way_and_tries_a_failed_one_again_so_a_send_reaches_its_prompt_source', async () => {
  const { shell } = setup();
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  // the shell's own later part (its palette sources and keys, builtins-later.ts) first: then nothing is on its way
  await shell.context.laterReady?.();
  expect(shell.context.laterReady?.()).toBeUndefined();
  let tries = 0;
  const flaky = (): Promise<{ default: PluginUiLater }> => {
    tries += 1;
    if (tries === 1) return Promise.reject(new Error('chunk 404'));
    return Promise.resolve({ default: { prompt: [{ id: 'flaky.goal', trigger: '/', items: () => [], send: () => true }] } });
  };
  shell.register({ id: 'wisp.flaky', later: flaky });
  // on its way: a send waits
  const first = shell.context.laterReady?.();
  expect(first).toBeInstanceOf(Promise);
  await first;
  expect(shell.context.promptSources()().map((p) => p.id)).not.toContain('flaky.goal');
  // the next send tries it again, and waits for it
  await shell.context.laterReady?.();
  expect(tries).toBe(2);
  expect(shell.context.promptSources()().map((p) => p.id)).toContain('flaky.goal');
  expect(shell.context.laterReady?.()).toBeUndefined();
  stop();
  shell.dispose();
});
