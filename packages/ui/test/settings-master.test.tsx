// SettingsMasterDetail (settings/SettingsMasterDetail.tsx, Settings → Agents): the list beside the picked item's
// detail; a press or ↑↓ picks, the detail follows, the search is the caller's, an empty list says so.
import { createSignal, Show } from 'solid-js';
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { SettingsMasterDetail, type SettingsMasterItem } from '../src/settings/SettingsMasterDetail.tsx';

const ALL: SettingsMasterItem[] = [
  { id: 'general', label: 'General', icon: 'settings' },
  { id: 'claude', label: 'Claude Code', sub: 'Ready', dot: 'good' },
  { id: 'cursor', label: 'Cursor', sub: 'Signed out', dot: 'warn' },
];

test('settings master detail: picking an item shows its detail; arrows move the pick; an empty search says so', async () => {
  const [picked, setPicked] = createSignal('general');
  const [q, setQ] = createSignal('');
  const items = (): SettingsMasterItem[] => ALL.filter((i) => i.label.toLowerCase().includes(q().toLowerCase()));
  const host = mount(() => (
    <SettingsMasterDetail label="Agents" items={items()} selected={picked()} onSelect={setPicked} search={{ placeholder: 'Search', value: q(), onInput: setQ }} empty="Nothing matches">
      <Show when={picked()} keyed>
        {(id) => <p data-detail={id}>{id}</p>}
      </Show>
    </SettingsMasterDetail>
  ));
  const row = (id: string): HTMLButtonElement | null => host.querySelector(`[data-kit="settings-master-item"][data-id="${id}"]`);
  expect(host.querySelector('[data-detail]')?.getAttribute('data-detail')).toBe('general');
  row('cursor')?.click();
  expect(host.querySelector('[data-detail]')?.getAttribute('data-detail')).toBe('cursor');
  expect(row('cursor')?.getAttribute('aria-selected')).toBe('true');
  expect(host.querySelector('[data-kit="settings-master-dot"][data-tone="warn"]')).not.toBeNull();
  row('cursor')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
  expect(picked()).toBe('claude');
  row('claude')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
  expect(picked(), 'round to the last').toBe('general');
  const input = host.querySelector<HTMLInputElement>('[data-kit="settings-master-search"] input');
  if (input) {
    input.value = 'zzz';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }
  expect(host.querySelectorAll('[data-kit="settings-master-item"]').length).toBe(0);
  expect(host.querySelector('[data-kit="settings-master-empty"]')?.textContent).toBe('Nothing matches');
});

// Machines (owner's pick B): no search, a heading over each run of a group, and `stack` (a phone's push): a pick opens
// the detail over the list, Back shows the list again.
test('settings master detail: groups head their runs; stack opens on a pick and Back closes it; no search box without search', () => {
  const [picked, setPicked] = createSignal('here');
  const items: SettingsMasterItem[] = [
    { id: 'here', label: 'This computer', group: 'Here' },
    { id: 'cloud', label: 'Cloud', group: 'Machines', dot: 'good' },
    { id: 'add', label: 'Add a machine', icon: 'plus', group: 'Machines' },
  ];
  const host = mount(() => (
    <SettingsMasterDetail label="Machines" items={items} selected={picked()} onSelect={setPicked} empty="" stack={{ back: 'Machines' }}>
      <p data-detail={picked()}>{picked()}</p>
    </SettingsMasterDetail>
  ));
  const root = host.querySelector('[data-kit="settings-master"]');
  expect(host.querySelector('[data-kit="settings-master-search"]')).toBeNull();
  expect([...host.querySelectorAll('[data-kit="settings-master-group"]')].map((g) => g.textContent)).toEqual(['Here', 'Machines']);
  expect(root?.hasAttribute('data-stack')).toBe(true);
  expect(root?.hasAttribute('data-open'), 'the list first').toBe(false);
  host.querySelector<HTMLButtonElement>('[data-id="cloud"]')?.click();
  expect(picked()).toBe('cloud');
  expect(root?.hasAttribute('data-open')).toBe(true);
  host.querySelector<HTMLButtonElement>('[data-kit="settings-master-back"]')?.click();
  expect(root?.hasAttribute('data-open'), 'Back shows the list').toBe(false);
  expect(picked(), 'and keeps the pick').toBe('cloud');
});
