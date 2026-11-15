// @vitest-environment happy-dom
// ADR-0065 step 14, the Crew tab in the phone apps (design/mobile.md §A.2 Crew), through the real client, shell and the
// crew plugin against the mock peer's crew at work (seed `crew`: Grace asks you something, Hopper has a report you
// haven't read, Linus and Ada work, Turing is paused). The Crew destination is the platform's screen: the root bar
// (Crew, Org, Add member, ⋮ / ⋯), the members as rows most needed first, each pushing its page (Back to Crew; Jobs ·
// Brief · Wakes; New job); Org pushed with Back. A phone's browser keeps v1's Org page.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId, type Client } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';

const settle = async (n = 30): Promise<void> => {
  for (let i = 0; i < n; i++) await new Promise((r) => setTimeout(r, 0));
};
/** until `ok`: the lazy chunks (the crew's pages, the phone screen) land a beat after the navigation (3 s at most) */
const until = async (ok: () => boolean): Promise<void> => {
  for (let i = 0; i < 300 && !ok(); i++) await new Promise((r) => setTimeout(r, 10));
};

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

const PLUGINS = new Set(['wisp.crew', 'wisp.shell-list', 'wisp.shell-task']);

async function start(platform: Platform): Promise<{
  client: Client;
  sent: { cmd: string; args: Record<string, unknown> }[];
}> {
  const mock = createMockPeer({ epoch: 'e1', crew: true });
  const sent: { cmd: string; args: Record<string, unknown> }[] = [];
  const { COMMANDS } = await import('../dev/mock-commands.ts');
  for (const cmd of ['crew.assign', 'crew.update']) {
    const fn = COMMANDS[cmd];
    mock.handle(cmd, (a, ctx) => (sent.push({ cmd, args: a as Record<string, unknown> }), fn ? fn(a, ctx) : { ops: [] }));
  }
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({
    client,
    toaster: createToaster(),
    platform,
    home,
    window: window as unknown as ShellWindow,
  });
  for (const { def } of pluginUis()) if (def && PLUGINS.has(def.id)) shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  await settle();
  return { client, sent };
}

const phone = (kind: 'android' | 'ios'): Platform => nullPlatform({ kind, phone: true, mobile: true });
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const CREW = { t: 'openView', id: 'crew.org', params: null } as const;
const members = (): string[] => [...document.querySelectorAll('[data-part~="crew.phone.member"] [data-kit="list-row-title"]')].map((e) => e.textContent ?? '');
const open = async (name: string): Promise<void> => {
  const row = [...document.querySelectorAll<HTMLElement>('[data-part~="crew.phone.member"]')].find((r) => r.textContent?.includes(name));
  expect(row, name).toBeDefined();
  row?.querySelector<HTMLElement>('[data-kit="list-row-hit"]')?.click();
  await until(() => q('[data-part~="crew.phone.member-bar"]') !== null);
  await settle();
};

for (const look of ['android', 'ios'] as const) {
  test(`crew_tab_on_${look}_is_the_platform_screen_listing_members_most_needed_first`, async () => {
    const { client } = await start(phone(look));
    client.nav.navigate(CREW as never);
    await until(() => members().length > 0);
    expect(q('[data-part~="crew.phone.bar"]')?.getAttribute('data-kind')).toBe('root');
    expect(q('[data-part~="crew.phone.bar"] [data-kit="app-bar-title"]')?.textContent).toContain('Crew');
    // asks you, then the unread report, then working, then idle (the stories row's order)
    expect(members().slice(0, 2)).toEqual(['Grace', 'Hopper']);
    expect(members()).toContain('Turing');
    expect(q('[data-part~="crew.phone.org"]'), 'Org in the bar').not.toBeNull();
    expect(q('[data-part~="crew.phone.add"]'), 'Add member in the bar').not.toBeNull();
    // the desktop chart isn't the root on a phone
    expect(q('[data-kit="org-view"]')).toBeNull();
  });

  test(`a_member_on_${look}_pushes_its_page_with_back_tabs_and_new_job`, async () => {
    const { client, sent } = await start(phone(look));
    client.nav.navigate(CREW as never);
    await until(() => members().length > 0);
    await open('Grace');
    expect(q('[data-part~="crew.phone.member-bar"]')?.getAttribute('data-kind')).toBe('page');
    expect(q('[data-part~="crew.phone.member-bar"] [data-kit="app-bar-title"]')?.textContent).toContain('Grace');
    const tabs = [...document.querySelectorAll('[data-part~="crew.phone.tabs"] [role="tab"]')].map((e) => e.textContent ?? '');
    expect(tabs.map((x) => x.replace(/\d+$/, '').trim())).toEqual(['Jobs', 'Brief', 'Wakes']);
    expect(document.querySelectorAll('[data-part~="crew.phone.job"]').length, 'her jobs').toBeGreaterThan(0);
    // New job: the reply box empties at once and the job is assigned to her
    const box = q('[data-part~="crew.phone.composer"] textarea') ?? q('textarea');
    expect(box).not.toBeNull();
    (box as HTMLTextAreaElement).value = 'Look at the flaky test';
    box?.dispatchEvent(new InputEvent('input', { bubbles: true }));
    await settle(3);
    const send = [...document.querySelectorAll<HTMLButtonElement>('[data-part~="composer.send"], [data-part~="crew.phone.composer"] button')].find(
      (b) => b.getAttribute('aria-label') === 'New job' || b.textContent?.includes('New job'),
    );
    expect(send, 'New job').toBeDefined();
    send?.click();
    await until(() => sent.some((s) => s.cmd === 'crew.assign'));
    expect(sent.find((s) => s.cmd === 'crew.assign')?.args['prompt']).toBe('Look at the flaky test');
    expect((box as HTMLTextAreaElement).value).toBe('');
    // Back to Crew
    q('[data-part~="crew.phone.member-bar.back"]')?.click();
    await until(() => q('[data-part~="crew.phone.bar"]') !== null);
    expect(q('[data-part~="crew.phone.member-bar"]')).toBeNull();
  });
}

test('the_member_page_tabs_show_brief_notebook_and_wakes_and_pause_is_one_tap', async () => {
  const { client, sent } = await start(phone('android'));
  client.nav.navigate(CREW as never);
  await until(() => members().length > 0);
  await open('Ada');
  const tab = (name: string): void =>
    [...document.querySelectorAll<HTMLElement>('[data-part~="crew.phone.tabs"] [role="tab"]')].find((e) => e.textContent?.startsWith(name))?.click();
  tab('Brief');
  await until(() => q('[data-part~="crew.phone.brief"]') !== null);
  expect(q('[data-part~="crew.phone.notebook"]'), 'the notebook (F-crew-6)').not.toBeNull();
  expect(q('[data-part~="crew.phone.facts"]')).not.toBeNull();
  tab('Wakes');
  await until(() => q('[data-part~="crew.phone.wakes"]') !== null);
  expect(q('[data-part~="crew.phone.wake.events"]'), 'Also starts when (F-crew-7)').not.toBeNull();
  expect(q('[data-part~="crew.phone.checkin"]')).not.toBeNull();
  q('[data-part~="crew.phone.pause"]')?.click();
  await until(() => sent.some((s) => s.cmd === 'crew.update'));
  expect(sent.find((s) => s.cmd === 'crew.update')?.args['paused']).toBe(true);
});

test('org_is_pushed_from_the_crew_root_with_back', async () => {
  const { client } = await start(phone('ios'));
  client.nav.navigate(CREW as never);
  await until(() => members().length > 0);
  q('[data-part~="crew.phone.org"]')?.click();
  await until(() => q('[data-part~="crew.phone.org-bar"]') !== null);
  expect(q('[data-part~="crew.phone.org-bar"]')?.getAttribute('data-kind')).toBe('page');
  await until(() => q('[data-part~="crew.org"]') !== null || q('[data-kit^="org"]') !== null);
  expect(q('[data-part~="crew.org"]') ?? q('[data-kit^="org"]'), 'the chart').not.toBeNull();
  q('[data-part~="crew.phone.org-bar.back"]')?.click();
  await until(() => q('[data-part~="crew.phone.bar"]') !== null);
  expect(members().length).toBeGreaterThan(0);
});

test('a_phones_browser_keeps_v1s_org_page_on_crew', async () => {
  const { client } = await start(nullPlatform({ mobile: true }));
  client.nav.navigate(CREW as never);
  await until(() => q('[data-kit^="org"]') !== null);
  await settle();
  expect(q('[data-part~="crew.phone.bar"]')).toBeNull();
  expect(q('[data-kit^="org"]')).not.toBeNull();
});
