// doc924a: a search hit opened the right pane at best and left you to find the row. Settings opened with `focus` on
// a key scrolls to that row once the pane is built, lights it for a moment and focuses its control; a key with no
// row there does nothing.
import { afterEach, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import type { PluginClient } from '@wisp/client';
import { FOUND_MS, SettingsGroup, SwitchSetting } from '@wisp/ui';
import { SettingsView } from '../src/frame.tsx';
import type { SettingsParams } from '../src/types.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
  vi.useRealTimers();
});

function Pane(): ReturnType<typeof SettingsGroup> {
  return (
    <SettingsGroup>
      <SwitchSetting title="Desktop" source={{ value: () => false, set: () => undefined, key: 'settings.notify' }} />
      <SwitchSetting title="Accept channels" source={{ value: () => false, set: () => undefined, key: 'channels.acceptChannels' }} />
    </SettingsGroup>
  );
}

function mount(first: SettingsParams) {
  const client = { navigate: () => undefined, home: 'laptop' } as unknown as PluginClient;
  const entry = { pane: { id: 'notifications', group: 'general', title: 'Notifications', icon: 'bell', component: Pane }, client } as never;
  const [params, setParams] = createSignal<SettingsParams>(first);
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <SettingsView client={client} params={params()} deps={{ panes: () => [entry], t: (k) => k }} />, host);
  const row = (key: string): HTMLElement | null => host.querySelector(`[data-setting-key="${key}"]`);
  return { row, setParams };
}

test('settings_opened_on_a_setting_scrolls_to_its_row_lights_it_and_focuses_its_control', () => {
  vi.useFakeTimers();
  const { row, setParams } = mount({ pane: 'notifications', focus: 'channels.acceptChannels' });
  const hit = row('channels.acceptChannels');
  expect(hit).not.toBeNull();
  expect(hit?.hasAttribute('data-found')).toBe(true);
  expect(hit?.contains(document.activeElement)).toBe(true);
  expect(row('settings.notify')?.hasAttribute('data-found')).toBe(false);
  vi.advanceTimersByTime(FOUND_MS + 10);
  expect(hit?.hasAttribute('data-found')).toBe(false); // the light goes out
  // the next hit in the same pane (the pane is kept): its row this time
  setParams({ pane: 'notifications', focus: 'settings.notify' });
  expect(row('settings.notify')?.hasAttribute('data-found')).toBe(true);
  expect(row('settings.notify')?.contains(document.activeElement)).toBe(true);
});

test('a_focus_key_with_no_row_in_the_pane_does_nothing', () => {
  const { row } = mount({ pane: 'notifications', focus: 'git.cloneDir' });
  expect(row('channels.acceptChannels')?.hasAttribute('data-found')).toBe(false);
  expect(document.querySelector('[data-found]')).toBeNull();
});
