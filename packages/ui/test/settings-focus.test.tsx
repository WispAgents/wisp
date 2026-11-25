// Settings audit r1: a setting found by Settings' search opened its pane at the top, the row nowhere marked. The row
// the search asked for (SettingsFocus) scrolls into view, flashes and takes its control's focus; the others don't.
// And Settings → Agents' list (SettingsSplit) walks with Up, Down, Home and End, and Enter goes into the detail.
import { expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount, until } from './helpers.tsx';
import { SettingsFocus, SettingsRow } from '../src/composites/Section.tsx';
import { SettingsSplit } from '../src/settings/SettingsSplit.tsx';

test('the_row_the_search_asked_for_scrolls_flashes_and_takes_focus_settings_audit_r1', async () => {
  const seen: string[] = [];
  Element.prototype.scrollIntoView = vi.fn(function (this: Element) {
    seen.push(this.getAttribute('data-key') ?? '');
  });
  const host = mount(() => (
    <SettingsFocus setting="worktrees.projectAsk">
      <SettingsRow settingKey="git.maskEnvFiles" title="Mask" control={<button type="button">a</button>} />
      <SettingsRow settingKey="worktrees.projectAsk" title="Ask" control={<button type="button">b</button>} />
    </SettingsFocus>
  ));
  const row = host.querySelector<HTMLElement>('[data-key="worktrees.projectAsk"]');
  await until(() => row?.hasAttribute('data-found') === true);
  expect(seen).toEqual(['worktrees.projectAsk']);
  expect(row?.hasAttribute('data-found')).toBe(true);
  expect(document.activeElement?.textContent).toBe('b');
  expect(host.querySelector('[data-key="git.maskEnvFiles"]')?.hasAttribute('data-found')).toBe(false);
});

test('the_split_list_walks_with_its_keys_and_enter_goes_into_the_detail_settings_audit_r1', () => {
  const [at, setAt] = createSignal('all');
  const host = mount(() => (
    <SettingsSplit
      label="Harnesses"
      items={[
        { id: 'all', label: 'All' },
        { id: 'claude', label: 'Claude Code', dot: { tone: 'good', label: 'Ready' }, ring: { value: 0.4, label: 'Week: 40% used' } },
        { id: 'grok', label: 'Grok', dot: { tone: 'neutral', label: 'Disabled' } },
      ]}
      current={at()}
      onPick={setAt}
      view="list"
      onBack={() => {}}
      backLabel="Harnesses"
      detailTitle="Detail"
    >
      <p>detail</p>
    </SettingsSplit>
  ));
  const list = host.querySelector<HTMLElement>('[data-kit="settings-split-items"]') as HTMLElement;
  const key = (k: string): void => void list.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));
  key('ArrowDown');
  expect(at()).toBe('claude');
  key('End');
  expect(at()).toBe('grok');
  key('Home');
  expect(at()).toBe('all');
  key('ArrowUp');
  expect(at()).toBe('all');
  key('Enter');
  expect(document.activeElement?.getAttribute('data-kit')).toBe('settings-split-detail');
  // its dot and its ring say what they mean
  expect(host.querySelector('[data-item="claude"] [data-kit="settings-split-dot"]')?.getAttribute('aria-label')).toBe('Ready');
  expect(host.querySelector('[data-item="claude"] [data-kit="meter"]')?.getAttribute('data-shape')).toBe('ring');
  expect(host.querySelector('[data-item="grok"] [data-kit="meter"]')).toBeNull();
});
