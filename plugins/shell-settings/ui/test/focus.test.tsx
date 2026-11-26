// doc924a: a search hit opened the right pane at best and left you to find the row. Settings opened with `focus` on
// a key scrolls to that row once the pane is built, lights it for a moment and focuses its control; a key with no
// row there does nothing.
import { afterEach, expect, test, vi } from 'vitest';
import { createEffect, createSignal, on, Show } from 'solid-js';
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

// new-agents-filter + doc924a review: a hit for another harness while Agents is open keeps the pane (samePane) and
// changes its filter; the pane re-picks (as Agents does) and the new harness's row is the one lit and focused, never
// the last harness's row of the same kind
function AgentsLike(props: { readonly filter?: string }): ReturnType<typeof SettingsGroup> {
  const [picked, setPicked] = createSignal(props.filter ?? 'general');
  createEffect(on(() => props.filter, (f) => void (f && setPicked(f)), { defer: true }));
  return (
    <Show when={picked()} keyed>
      {(id) => (
        <SettingsGroup>
          <SwitchSetting title="Default model" source={{ value: () => false, set: () => undefined, key: `agents.${id}.model` }} />
          <SwitchSetting title="Where automations stop" source={{ value: () => false, set: () => undefined, key: `budget.autoStop${id[0]?.toUpperCase()}${id.slice(1)}` }} />
        </SettingsGroup>
      )}
    </Show>
  );
}

test('agents_re_picked_by_a_new_filter_lights_and_focuses_the_new_harness_row', async () => {
  const client = { navigate: () => undefined, home: 'laptop' } as unknown as PluginClient;
  const entry = { pane: { id: 'agents', group: 'general', title: 'Agents', icon: 'bot', component: AgentsLike }, client } as never;
  const [params, setParams] = createSignal<SettingsParams>({ pane: 'agents', filter: 'claude', focus: 'agents.claude.model' });
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <SettingsView client={client} params={params()} deps={{ panes: () => [entry], t: (k) => k }} />, host);
  const row = (key: string): HTMLElement | null => host.querySelector(`[data-setting-key="${key}"]`);
  await vi.waitFor(() => expect(row('agents.claude.model')?.hasAttribute('data-found')).toBe(true));
  // another harness's stop line (budget.autoStopCodex, routed to Agents by harnessOfKey): the same pane, a new filter
  setParams({ pane: 'agents', filter: 'codex', focus: 'budget.autoStopCodex' });
  await vi.waitFor(() => expect(row('budget.autoStopCodex')?.hasAttribute('data-found')).toBe(true));
  expect(row('budget.autoStopCodex')?.contains(document.activeElement)).toBe(true);
  expect(row('agents.claude.model'), 'the last harness detail is gone').toBeNull();
  // and a detail row of the new harness
  setParams({ pane: 'agents', filter: 'codex', focus: 'agents.codex.model' });
  await vi.waitFor(() => expect(row('agents.codex.model')?.hasAttribute('data-found')).toBe(true));
  expect(row('agents.codex.model')?.contains(document.activeElement)).toBe(true);
});
