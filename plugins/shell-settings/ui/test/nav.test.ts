import { expect, test } from 'vitest';
import { groupPanes, type PaneEntry } from '../src/nav.ts';
import { offeredPanes, phoneGroups } from '../src/phone-nav.ts';

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

test('phone_settings_are_sections_by_group_in_the_contracts_order_each_named_with_v1s_order_inside', () => {
  const groups = phoneGroups(
    [pane('privacy', 'advanced', 30), pane('theme', 'general', 5), pane('agents', 'agents', 10), pane('general', 'general', 10), pane('machine', 'computers', 50), pane('auto', 'agents', 20), pane('mine', 'zeta', 1)],
    (k) => k,
    (g) => `G:${g}`,
  );
  expect(groups.map((g) => g.label)).toEqual(['G:general', 'G:agents', 'G:computers', 'G:advanced', 'G:zeta']);
  expect(groups.map((g) => g.items.map((i) => i.id))).toEqual([['general', 'theme'], ['agents', 'auto'], ['machine'], ['privacy'], ['mine']]);
  expect(phoneGroups([], (k) => k, (g) => g)).toEqual([]);
});

test('a_phone_app_offers_every_pane_but_the_keyboard_shortcuts', () => {
  const all = [pane('general', 'general'), pane('keys', 'general'), pane('agents', 'agents')];
  expect(offeredPanes(all, true).map((e) => e.pane.id)).toEqual(['general', 'agents']);
  expect(offeredPanes(all, false).map((e) => e.pane.id)).toEqual(['general', 'keys', 'agents']);
});
