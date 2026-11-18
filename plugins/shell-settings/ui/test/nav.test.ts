import { expect, test } from 'vitest';
import { groupPanes, samePane, type PaneEntry } from '../src/nav.ts';
import { filterSections, offeredPanes, sectionedPanes } from '../src/phone-nav.ts';
import { settingsPalette } from '../src/index.ts';

const pane = (id: string, group: string, order = 50): PaneEntry => ({ pane: { id, group, title: `t.${id}`, icon: 'settings', order, component: () => null }, client: {} as never });

test('settings_nav_is_v1s_flat_list_in_v1s_order_with_v2s_own_panes_after', () => {
  const groups = groupPanes(
    [pane('privacy', 'advanced', 30), pane('theme', 'general', 5), pane('agents', 'agents', 10), pane('general', 'general', 10), pane('updates', 'advanced', 10), pane('plugins', 'advanced', 5), pane('machine', 'computers', 50), pane('usage', 'advanced', 20), pane('auto', 'agents', 20), pane('keys', 'general', 40)],
    (k) => k,
  );
  expect(groups).toHaveLength(1);
  expect(groups[0]?.label).toBeUndefined();
  expect(groups[0]?.items.map((i) => i.id)).toEqual(['general', 'agents', 'auto', 'plugins', 'machine', 'usage', 'privacy', 'theme', 'keys', 'updates']);
  expect(groupPanes([], (k) => k)).toEqual([]);
});

test('settings_are_sections_by_group_in_the_contracts_order_each_named_with_v1s_order_inside', () => {
  const groups = sectionedPanes(
    [pane('privacy', 'advanced', 30), pane('theme', 'general', 5), pane('agents', 'agents', 10), pane('general', 'general', 10), pane('machine', 'computers', 50), pane('auto', 'agents', 20), pane('mine', 'zeta', 1)],
    (k) => k,
    (g) => `G:${g}`,
  );
  expect(groups.map((g) => g.label)).toEqual(['G:general', 'G:agents', 'G:computers', 'G:advanced', 'G:zeta']);
  expect(groups.map((g) => g.items.map((i) => i.id))).toEqual([['general', 'theme'], ['agents', 'auto'], ['machine'], ['privacy'], ['mine']]);
  expect(sectionedPanes([], (k) => k, (g) => g)).toEqual([]);
});

test('a_phone_app_offers_every_pane_but_the_keyboard_shortcuts', () => {
  const all = [pane('general', 'general'), pane('keys', 'general'), pane('agents', 'agents')];
  expect(offeredPanes(all, true).map((e) => e.pane.id)).toEqual(['general', 'agents']);
  expect(offeredPanes(all, false).map((e) => e.pane.id)).toEqual(['general', 'keys', 'agents']);
});

test('cmd_k_finds_every_settings_pane_by_name_and_lede_and_opens_it_TRY_28', () => {
  const words: Record<string, string> = { 't.sandboxes': 'Sandboxes', 't.sandboxes.lede': 'Boxes on your grid', 't.keys': 'Keyboard shortcuts' };
  const deps = (phone: boolean) => ({ panes: () => [pane('sandboxes', 'computers'), pane('keys', 'general')], t: (k: string) => words[k] ?? k, phone });
  const went: unknown[] = [];
  const ctx = { client: { navigate: (i: unknown) => went.push(i) }, nav: {}, focused: undefined } as never;
  const items = settingsPalette(deps(false)).items(ctx);
  expect(items.map((i) => [i.label, i.keywords])).toEqual([
    ['Sandboxes', 'settings sandboxes Boxes on your grid'],
    ['Keyboard shortcuts', 'settings keys '],
  ]);
  items[0]?.run?.();
  expect(went).toEqual([{ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet', params: { pane: 'sandboxes' } } }]);
  expect(settingsPalette(deps(true)).items(ctx).map((i) => i.label)).toEqual(['Sandboxes']);
  expect(settingsPalette(deps(false)).items({ ...(ctx as object), narrowed: true } as never)).toEqual([]);
});

test('settings_search_keeps_the_panes_whose_name_section_or_lede_has_every_word_TRY_28', () => {
  const groups = sectionedPanes([pane('general', 'general'), pane('sandboxes', 'computers'), pane('cloud', 'computers'), pane('usage', 'agents')], (k) => ({ 't.general': 'General', 't.sandboxes': 'Sandboxes', 't.cloud': 'Cloud', 't.usage': 'Usage' })[k] ?? k, (g) => g[0]?.toUpperCase() + g.slice(1));
  const about = (id: string): string => (id === 'usage' ? 'Spending limits per day' : '');
  const ids = (q: string) => filterSections(groups, q, about).map((g) => [g.label, g.items.map((i) => i.id)]);
  expect(ids('sand')).toEqual([['Computers', ['sandboxes']]]);
  expect(ids('computers')).toEqual([['Computers', ['cloud', 'sandboxes']]]);
  expect(ids('LIMITS day')).toEqual([['Agents', ['usage']]]);
  expect(ids('nothing like it')).toEqual([]);
  expect(ids('  ')).toHaveLength(groups.length);
});

// QA setup audit F8: flipping a setting (Settings → Agents' "Ask before running a changed harness") remade the whole
// open pane, every row and its switch new, because the list of panes is made again when a setting changes and the
// sheet keyed the pane on its entry. The open pane is the same while its id, its component and its plugin are.
test('the_open_pane_is_the_same_while_its_id_component_and_plugin_are', () => {
  const component = (): null => null;
  const client = {} as PaneEntry['client'];
  const a: PaneEntry = { pane: { id: 'agents', group: 'agents', title: 't', icon: 'agent', component }, client };
  const again: PaneEntry = { pane: { ...a.pane }, client };
  expect(samePane(a, again)).toBe(true);
  expect(samePane(a, { pane: { ...a.pane, id: 'usage' }, client })).toBe(false);
  expect(samePane(a, { pane: { ...a.pane, component: (): null => null }, client })).toBe(false);
  expect(samePane(a, { pane: a.pane, client: {} as PaneEntry['client'] })).toBe(false);
  expect(samePane(undefined, undefined)).toBe(true);
  expect(samePane(a, undefined)).toBe(false);
});
