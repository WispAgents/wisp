// @vitest-environment happy-dom
// ADR-0065 step 14 on phones (design/mobile.md §A.2 Settings, My Stuff), through the real client, shell and plugins
// (shell-settings, stuff) against the mock peer. Settings, opened as from the me menu, is the platform's pushed pages
// over the whole screen (the list of panes, a pane pushed with Back), without the keyboard-only Keys pane (v1
// F-set-6); My Stuff on the Stuff destination is the platform's screen with the mock's three kept things, Share hands
// the platform's share sheet the thing, Remove leaves at once with Undo and runs `stuff.remove` once the toast's time
// is up. A phone's browser keeps v1's Settings and My Stuff.
import { afterEach, expect, test, vi } from 'vitest';
import { createClient, peerId, type Timers } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { shellSettingsUi } from '@wisp-plugin/shell-settings-ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';
import { hop, until } from './until.ts';

/** Undo's time (the stuff plugin's UNDO_MS) */
const UNDO_MS = 5000;
/** the client's timers, real but for those of Undo's length (Undo's, and the peer link's watchdog): they wait until the
 * test says that time has passed, then all run as they would have (no 5 s on the test's clock) */
function undoTimers(): Timers & { up(): void; readonly held: number } {
  const held = new Map<number, () => void>();
  let n = 0;
  return {
    now: () => Date.now(),
    setTimeout(fn, ms) {
      if (ms !== UNDO_MS) return globalThis.setTimeout(fn, ms);
      held.set(++n, fn);
      return `undo:${n}`;
    },
    clearTimeout(h) {
      if (typeof h === 'string' && h.startsWith('undo:')) held.delete(Number(h.slice(5)));
      else globalThis.clearTimeout(h as Parameters<typeof globalThis.clearTimeout>[0]);
    },
    up() {
      const fns = [...held.values()];
      held.clear();
      for (const fn of fns) fn();
    },
    get held() {
      return held.size;
    },
  };
}

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  vi.useRealTimers();
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

type PaneEntry = Parameters<typeof shellSettingsUi>[0] extends { panes: () => readonly (infer E)[] } ? E : never;
const PLUGINS = new Set(['wisp.shell-settings', 'wisp.stuff', 'wisp.shell-list']);

async function start(platform: Platform) {
  const mock = createMockPeer({ epoch: 'e1' });
  const sent: string[] = [];
  const { COMMANDS } = await import('../dev/mock-commands.ts');
  for (const cmd of ['stuff.remove']) {
    const fn = COMMANDS[cmd];
    if (fn) mock.handle(cmd, (a, ctx) => (sent.push(`${cmd}:${String((a as { id?: string }).id)}`), fn(a, ctx)));
  }
  const home = peerId('local');
  const timers = undoTimers();
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390, timers });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def && PLUGINS.has(def.id)) shell.register(def);
  // Settings as the apps register it (apps/web and mobile/web main.tsx): every plugin's panes, and the phone flag
  const panes = (): PaneEntry[] =>
    shell
      .settingsPanes()()
      .flatMap((pane) => {
        const c = shell.pluginClient(pane.plugin);
        return c ? [{ pane, client: c }] : [];
      });
  shell.register(shellSettingsUi({ panes, t: (key, vars) => client.i18n.t(key, vars), phone: platform.phone }));
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, shell, root, sent, timers };
}

const phone = (kind: 'android' | 'ios', more: Partial<Platform> = {}): Platform => ({ ...nullPlatform({ kind, phone: true, mobile: true }), ...more });
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const SETTINGS = { t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet', params: {} } } as const;
const STUFF = { t: 'openView', id: 'stuff.page', params: null } as const;
/** a kept thing's ⋯ menu, then one of its entries (the menu's machine loads on the first press) */
async function act(title: string, label: string): Promise<void> {
  q(`[aria-label="More for ${title}"]`)?.click();
  // the open menu's entry (every menu's entries are in the page, hidden, until it opens)
  const item = (): HTMLElement | undefined => [...document.querySelectorAll<HTMLElement>('[role="menu"][data-state="open"] [role="menuitem"]')].find((e) => e.textContent?.trim() === label);
  await until(() => item() !== undefined);
  expect(item(), label).toBeDefined();
  // as a finger does: Zag's menu picks an item on its pointer up, after the pointer is over it
  const el = item() as HTMLElement;
  const init = { bubbles: true, cancelable: true, button: 0, pointerType: 'touch', isPrimary: true };
  el.dispatchEvent(new PointerEvent('pointermove', init));
  el.dispatchEvent(new PointerEvent('pointerdown', init));
  el.dispatchEvent(new PointerEvent('pointerup', init));
  el.click();
}
const titles = (sel: string): string[] => [...document.querySelectorAll(`${sel} [data-kit="list-row-title"]`)].map((e) => e.textContent ?? '');

for (const look of ['android', 'ios'] as const) {
  test(`settings_on_${look}_is_a_full_screen_page_listing_the_panes_and_pushes_one_with_back`, async () => {
    const { client } = await start(phone(look));
    client.nav.navigate(SETTINGS as never);
    await until(() => q('[data-part~="settings.item"]') !== null);
    expect(q('[data-kit="dialog"][data-size="full"]'), 'over the whole screen').not.toBeNull();
    expect(q('[data-kit="dialog"] [data-kit="app-bar"]')?.getAttribute('data-kind')).toBe('page');
    const panes = titles('[data-part~="settings.nav"]');
    expect(panes).toContain('General');
    expect(panes, 'no shortcuts sheet in the phone apps (F-set-6)').not.toContain('Keys');
    const general = [...document.querySelectorAll<HTMLElement>('[data-part~="settings.item"]')].find((r) => r.textContent?.includes('General'));
    general?.querySelector<HTMLElement>('[data-kit="list-row-hit"]')?.click();
    await until(() => q('[data-part~="settings.pane-bar"]') !== null);
    expect(q('[data-part~="settings.pane-bar"] [data-kit="app-bar-title"]')?.textContent).toContain('General');
    await until(() => q('[data-kit="phone-settings-pane"] [data-kit="settings-group"]') !== null);
    expect(q('[data-kit="phone-settings-pane"] [data-kit="settings-group"]'), 'the pane as it is').not.toBeNull();
    q('[data-part~="settings.pane-bar.back"]')?.click();
    await until(() => q('[data-part~="settings.item"]') !== null);
    expect(q('[data-part~="settings.pane-bar"]')).toBeNull();
    q('[data-part~="settings.bar.back"]')?.click();
    await until(() => q('[data-kit="dialog"][data-size="full"]') === null);
    expect(client.nav.nav().overlays.length, 'Back leaves Settings').toBe(0);
  });

  test(`my_stuff_on_${look}_lists_whats_kept_as_the_platforms_screen`, async () => {
    const { client } = await start(phone(look));
    client.nav.navigate(STUFF as never);
    await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 3);
    expect(q('[data-part~="stuff.phone.bar"]')?.getAttribute('data-kind')).toBe('root');
    expect(q('[data-part="stuff.page"]'), 'not v1’s page').toBeNull();
    expect([...document.querySelectorAll('[data-kit="stuff-tile-name"] > span:first-child')].map((e) => e.textContent)).toEqual(['wisp v2 rewrite plan', 'Token budget dashboard', 'Settings, phone, dark']);
    // the kinds: Android's chips, iOS's segmented control
    expect(q(look === 'ios' ? '[data-kit="stuff-phone-kinds"] [data-kit="tabs"]' : '[data-kit="stuff-phone-kinds"] [data-variant="chip"]')).not.toBeNull();
  });
}

test('my_stuff_share_hands_the_platforms_share_sheet_the_thing', async () => {
  const shared: unknown[] = [];
  const { client } = await start(phone('android', { share: async (d) => (shared.push(d), 'shared') }));
  client.nav.navigate(STUFF as never);
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 3);
  // the menu's Share (a list row's swipe to the right runs the same)
  await act('wisp v2 rewrite plan', 'Share');
  await until(() => shared.length > 0);
  expect(shared[0]).toMatchObject({ title: 'wisp v2 rewrite plan' });
});

test('my_stuff_remove_leaves_at_once_with_undo_then_runs_stuff_remove', async () => {
  const { client, sent, timers } = await start(phone('ios'));
  client.nav.navigate(STUFF as never);
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 3);
  await act('Settings, phone, dark', 'Remove');
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 2);
  expect(sent, 'not yet: Undo is still up').toEqual([]);
  const undoButton = (): HTMLButtonElement | undefined => [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.trim() === 'Undo');
  await until(() => undoButton() !== undefined);
  expect(document.body.textContent).toContain('Removed Settings, phone, dark');
  const undo = undoButton();
  expect(undo, 'the toast’s Undo').toBeDefined();
  undo?.click();
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 3);
  expect(sent).toEqual([]);
  // again, and let the toast's time run out
  await act('Settings, phone, dark', 'Remove');
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 2);
  // Undo's time is up, then the removal
  expect(sent, 'not before Undo’s time').toEqual([]);
  timers.up();
  await until(() => sent.length > 0);
  expect(sent).toEqual(['stuff.remove:st3']);
});

test('my_stuff_undo_still_works_after_leaving_the_screen', async () => {
  const { client, sent, timers } = await start(phone('android'));
  client.nav.navigate(STUFF as never);
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 3);
  await act('Settings, phone, dark', 'Remove');
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 2);
  const waiting = timers.held;
  // to Tasks, Undo from the toast there, and back: it's kept, and nothing was removed
  client.nav.navigate({ t: 'home' });
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 0);
  const undo = (): HTMLButtonElement | undefined => [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.trim() === 'Undo');
  await until(() => undo() !== undefined);
  undo()?.click();
  client.nav.navigate(STUFF as never);
  await until(() => document.querySelectorAll('[data-kit="stuff-tile"]').length === 3);
  expect(document.querySelectorAll('[data-kit="stuff-tile"]').length).toBe(3);
  // Undo cleared the removal's timer: when Undo's time would be up, nothing runs
  expect(timers.held, 'Undo cleared the removal').toBe(waiting - 1);
  timers.up();
  await hop();
  expect(sent, 'Undo held after the screen went').toEqual([]);
});

test('a_phones_browser_keeps_v1s_my_stuff_and_settings', async () => {
  const { client } = await start(nullPlatform({ kind: 'browser', mobile: true }));
  client.nav.navigate(STUFF as never);
  await until(() => q('[data-part="stuff.page"]') !== null);
  expect(q('[data-kit="stuff-tile"]')).toBeNull();
  client.nav.navigate(SETTINGS as never);
  await until(() => q('[data-kit="settings-frame"]') !== null);
  expect(q('[data-kit="dialog"][data-size="full"]')).toBeNull();
  expect(q('[data-kit="phone-page"]'), 'v1’s sidebar of panes, not the phone pages').toBeNull();
});
