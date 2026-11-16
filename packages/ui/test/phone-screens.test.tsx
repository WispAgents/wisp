// ADR-0065 step 14 (design/mobile.md §A.2 Settings, My Stuff): the kit's phone screens. SettingsFrame draws the
// platform's pushed pages in the phone apps (the list under a page bar with Back, a pane pushed with Back to Settings)
// and v1's sidebar frame in the browser; StuffPhone is My Stuff's root screen (Android's filter chips, iOS's segmented
// control; tiles or swipeable rows, each with its ⋯ menu on screen, the swipes' twin); StuffPhoneApp an app over the
// whole screen with Close / Done.
import { beforeAll, describe, expect, it } from 'vitest';
import { createSignal, type JSX } from 'solid-js';
import { mount, tick, until } from './helpers.tsx';
import { PlatformLookProvider, SettingsFrame, StuffPhone, StuffPhoneApp, GESTURES, type Look, type SettingsNavGroup, type StuffPhoneGroup } from '../src/index.ts';
import { settingsPhone } from '../src/settings/settings-phone-load.ts';

const noop = (): void => {};
const as = (look: Look, view: () => JSX.Element): HTMLElement => mount(() => <PlatformLookProvider look={() => look}>{view()}</PlatformLookProvider>);
const GROUPS: SettingsNavGroup[] = [
  { id: 'general', items: [{ id: 'general', label: 'General', icon: 'settings' }, { id: 'notifications', label: 'Notifications', icon: 'bell' }] },
  { id: 'agents', label: 'Agents', items: [{ id: 'agents', label: 'Agents', icon: 'agent', alert: 'warn', alertLabel: 'An agent was not found' }] },
];

beforeAll(async () => {
  await settingsPhone.load();
});

function settings(look: Look, view: () => 'list' | 'pane', picked: string[] = [], up: () => void = noop, close: () => void = noop): HTMLElement {
  return as(look, () => (
    <SettingsFrame
      title="Settings"
      groups={GROUPS}
      current="general"
      onPick={(id) => picked.push(id)}
      paneTitle="General"
      view={view()}
      onUp={up}
      upLabel="Back"
      onClose={close}
      closeLabel="Close"
      backLabel="Tasks"
    >
      <p data-testid="pane">the pane</p>
    </SettingsFrame>
  ));
}

describe('Settings on phones', () => {
  for (const look of ['android', 'ios'] as const) {
    it(`settings_list_on_${look}_is_a_page_bar_with_back_and_a_pushed_row_per_pane_in_its_groups`, () => {
      const picked: string[] = [];
      let closed = 0;
      const host = settings(look, () => 'list', picked, noop, () => closed++);
      const bar = host.querySelector('[data-kit="app-bar"]');
      expect(bar?.getAttribute('data-kind'), 'a pushed page').toBe('page');
      expect(host.querySelector('[data-kit="settings-nav"]'), 'no browser sidebar').toBeNull();
      const groups = [...host.querySelectorAll('[data-kit="settings-group"]')];
      expect(groups.length).toBe(2);
      expect(groups[1]?.querySelector('[data-kit="settings-group-title"]')?.textContent).toBe('Agents');
      const rows = [...host.querySelectorAll<HTMLElement>('[data-part~="settings.frame.item"]')];
      expect(rows.map((r) => r.querySelector('[data-kit="list-row-title"]')?.textContent)).toEqual(['General', 'Notifications', 'Agents']);
      expect(host.querySelector('[data-kit="settings-nav-alert"]')?.getAttribute('aria-label')).toBe('An agent was not found');
      expect(host.querySelector('[data-testid="pane"]'), 'no pane under the list').toBeNull();
      rows[1]?.querySelector<HTMLElement>('[data-kit="list-row-hit"]')?.click();
      expect(picked).toEqual(['notifications']);
      host.querySelector<HTMLElement>('[data-part~="settings.frame.bar.back"]')?.click();
      expect(closed, 'Back leaves Settings').toBe(1);
    });

    it(`settings_pane_on_${look}_is_pushed_with_its_title_and_back_to_settings`, () => {
      let up = 0;
      const host = settings(look, () => 'pane', [], () => up++);
      expect(host.querySelector('[data-kit="app-bar-title"]')?.textContent).toContain('General');
      expect(host.querySelector('[data-testid="pane"]')?.textContent).toBe('the pane');
      expect(host.querySelector('[data-part~="settings.frame.item"]'), 'the list is under it').toBeNull();
      host.querySelector<HTMLElement>('[data-part~="settings.frame.pane-bar.back"]')?.click();
      expect(up).toBe(1);
    });
  }

  it('ios_settings_rows_push_with_a_chevron_and_back_names_the_screen_under', () => {
    const host = settings('ios', () => 'list');
    const row = host.querySelector('[data-part~="settings.frame.item"]');
    expect(row?.querySelector('[data-kit="list-row-chevron"], svg[data-icon="chevron"]'), 'a disclosure chevron').not.toBeNull();
    expect(host.querySelector('[data-part~="settings.frame.bar.back"]')?.textContent).toContain('Tasks');
  });

  it('settings_in_the_browser_keeps_v1s_sidebar_frame', () => {
    const host = mount(() => (
      <SettingsFrame title="Settings" groups={GROUPS} current="general" onPick={noop} paneTitle="General" view="list" onUp={noop} upLabel="Back" closeLabel="Close" />
    ));
    expect(host.querySelector('[data-kit="settings-frame"] [data-kit="settings-nav"]')).not.toBeNull();
    expect(host.querySelector('[data-kit="app-bar"]')).toBeNull();
    expect(host.querySelector('[data-kit="phone-page"]')).toBeNull();
  });

  it('a_pane_picked_on_a_phone_swaps_list_for_pane_in_place', async () => {
    const [view, setView] = createSignal<'list' | 'pane'>('list');
    const host = settings('android', view);
    expect(host.querySelector('[data-part~="settings.frame.item"]')).not.toBeNull();
    setView('pane');
    await tick();
    expect(host.querySelector('[data-testid="pane"]')).not.toBeNull();
    expect(host.querySelector('[data-kit="phone-page"]')?.getAttribute('data-view')).toBe('pane');
  });
});

const ITEMS: StuffPhoneGroup[] = [
  {
    id: 'today',
    label: 'Today',
    items: [
      { id: 'a', title: 'Rewrite plan', sub: 'Doc · 98 KB', when: '2h', icon: 'file', menu: [{ value: 'open', label: 'Open' }, { value: 'share', label: 'Share' }, { value: 'remove', label: 'Remove', danger: true }] },
      { id: 'b', title: 'Budget dashboard', sub: 'App · 14 KB', when: '3h', icon: 'app', menu: [{ value: 'open', label: 'Launch' }] },
    ],
  },
];

function stuff(look: Look, view: 'grid' | 'list', more: { opened?: string[]; swiped?: string[]; kinds?: string[]; empty?: boolean; added?: number[] } = {}): HTMLElement {
  return as(look, () => (
    <StuffPhone
      title="My Stuff"
      add={{ label: 'Add files', onSelect: () => more.added?.push(1) }}
      actionsLabel="My Stuff actions"
      menu={{ label: 'More', items: [{ value: 'grid', label: 'Cards' }], onSelect: noop }}
      search={{ value: '', label: 'Search My Stuff', placeholder: 'Search', onInput: noop }}
      kinds={{ label: 'Kinds', items: [{ id: 'all', label: 'All', count: 2 }, { id: 'doc', label: 'Docs', count: 1 }, { id: 'app', label: 'Apps', count: 1 }], value: 'all', onPick: (id) => more.kinds?.push(id) }}
      view={view}
      groups={more.empty ? [] : ITEMS}
      onOpen={(id) => more.opened?.push(id)}
      onMenu={noop}
      menuLabel={(n) => `More for ${n}`}
      swipe={{ right: { label: 'Share', icon: 'share', tone: 'accent' }, left: { label: 'Remove', icon: 'trash', tone: 'bad' }, onSwipe: (id, dir) => more.swiped?.push(`${id}:${dir}`) }}
      empty={more.empty ? { title: 'Keep what your agents make', body: 'Save from a reply', action: 'Add files', onAction: noop } : undefined}
      nothing="Nothing matches."
    />
  ));
}

describe('My Stuff on phones', () => {
  it('stuff_on_android_has_the_root_bar_filter_chips_and_two_column_tiles', () => {
    const kinds: string[] = [];
    const opened: string[] = [];
    const host = stuff('android', 'grid', { kinds, opened });
    expect(host.querySelector('[data-kit="app-bar"]')?.getAttribute('data-kind')).toBe('root');
    const chips = [...host.querySelectorAll<HTMLButtonElement>('[data-kit="stuff-phone-kinds"] [data-kit="button"][data-variant="chip"]')];
    expect(chips.map((c) => c.textContent)).toEqual(['All 2', 'Docs 1', 'Apps 1']);
    expect(chips[0]?.getAttribute('aria-pressed')).toBe('true');
    chips[1]?.click();
    expect(kinds).toEqual(['doc']);
    expect(host.querySelector('[data-kit="tabs"]'), 'no segmented control on Android').toBeNull();
    const tiles = [...host.querySelectorAll<HTMLElement>('[data-kit="stuff-tile"]')];
    expect(tiles.length).toBe(2);
    expect(host.querySelector('[data-kit="stuff-phone-grid"]')).not.toBeNull();
    tiles[0]?.querySelector<HTMLButtonElement>('[data-kit="stuff-tile-open"]')?.click();
    expect(opened).toEqual(['a']);
    expect(tiles[0]?.querySelector('[aria-label="More for Rewrite plan"]'), 'each tile has its ⋯ on screen').not.toBeNull();
  });

  it('stuff_add_files_is_androids_bar_icon_and_in_ioss_more_menu', async () => {
    const added: number[] = [];
    const host = stuff('android', 'grid', { added });
    const add = host.querySelector<HTMLElement>('[data-part~="stuff.phone.add"]');
    expect(add?.getAttribute('aria-label')).toBe('Add files');
    expect(add?.textContent, 'an icon, no label').toBe('');
    add?.click();
    expect(added).toEqual([1]);
    const ios = stuff('ios', 'grid');
    expect(ios.querySelector('[data-part~="stuff.phone.add"]'), 'iOS: in the ⋯ menu').toBeNull();
    ios.querySelector<HTMLElement>('[aria-label="More"]')?.click();
    const entry = (): Element | undefined => [...document.querySelectorAll('[data-kit="menu-item"]')].find((e) => e.textContent?.includes('Add files'));
    await until(() => entry() !== undefined);
    expect(entry(), 'Add files in ⋯').toBeDefined();
  });

  it('stuff_on_ios_draws_the_kinds_as_a_segmented_control', () => {
    const host = stuff('ios', 'grid');
    expect(host.querySelector('[data-kit="stuff-phone-kinds"] [data-kit="tabs"][data-variant="pill"]')).not.toBeNull();
    expect(host.querySelector('[data-kit="stuff-phone-kinds"] [data-variant="chip"]')).toBeNull();
    expect(host.querySelector('[data-kit="app-bar-search-field"]'), 'iOS search under the large title').not.toBeNull();
  });

  it('stuff_list_rows_swipe_share_right_and_remove_left_with_the_rows_menu_as_their_twin', () => {
    const host = stuff('android', 'list');
    const swipes = [...host.querySelectorAll<HTMLElement>('[data-kit="swipe"]')];
    expect(swipes.length).toBe(2);
    expect(swipes[0]?.getAttribute('data-gesture')).toBe('stuff.swipe');
    expect(swipes[0]?.getAttribute('data-twin')).toBe('stuff.phone.row.menu.trigger');
    expect(GESTURES['stuff.swipe'].twin).toEqual({ command: 'stuff.remove', parts: ['stuff.phone.row.menu.trigger'] });
    // the twin is on screen in the same row, by the exact name qa/platform.mjs's twinRule looks for
    const twin = swipes[0]?.querySelector('[data-part="stuff.phone.row.menu.trigger"]');
    expect(twin?.getAttribute('aria-label')).toBe('More for Rewrite plan');
  });

  it('stuff_search_opens_a_field_under_the_bar', async () => {
    const host = stuff('android', 'grid');
    expect(host.querySelector('[data-kit="stuff-phone-search"]')).toBeNull();
    host.querySelector<HTMLElement>('[data-part~="stuff.phone.bar.search"]')?.click();
    await tick();
    expect(host.querySelector('[data-kit="stuff-phone-search"] input[type="search"]')).not.toBeNull();
  });

  it('stuff_with_nothing_kept_shows_the_empty_state_with_add_files', () => {
    const host = stuff('ios', 'grid', { empty: true });
    expect(host.querySelector('[data-kit="empty-state-title"]')?.textContent).toBe('Keep what your agents make');
    expect(host.querySelector('[data-part~="stuff.phone.empty.add"]')?.textContent).toContain('Add files');
  });

  for (const [look, word] of [
    ['android', ''],
    ['ios', 'Done'],
  ] as const) {
    it(`a_launched_app_on_${look}_fills_the_screen_with_its_frame_and_close`, async () => {
      let closed = 0;
      as(look, () => <StuffPhoneApp title="Budget dashboard" src="about:blank" sandbox="allow-scripts" closeLabel="Close" doneLabel="Done" onClose={() => closed++} />);
      await tick();
      const dialog = document.querySelector('[data-kit="dialog"][data-size="full"]');
      expect(dialog, 'over the whole screen').not.toBeNull();
      expect(dialog?.querySelector('iframe[sandbox="allow-scripts"]')).not.toBeNull();
      const out = dialog?.querySelector<HTMLElement>('[data-part~="stuff.phone.app.bar.back"]');
      expect(out?.textContent).toBe(word);
      out?.click();
      expect(closed).toBe(1);
    });
  }
});
