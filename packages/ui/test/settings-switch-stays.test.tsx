// QA setup audit F8 (Settings → Agents, "Ask before running a changed harness"): a switch in a settings row was made
// again on every change: a new button each time, so it lost focus, its slide, and a second click landing while it
// was swapped. The row keeps its control; only the switch's state changes.
import { expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { SettingsRow, Switch } from '../src/index.ts';

test('a_settings_rows_switch_stays_the_same_element_as_it_flips', () => {
  const [on, setOn] = createSignal(false);
  const host = mount(() => <SettingsRow part="t.row" title="Ask first" control={<Switch aria-label="Ask first" checked={on()} onChange={setOn} />} />);
  const first = host.querySelector('[role="switch"]') as HTMLButtonElement;
  expect(first.getAttribute('aria-checked')).toBe('false');
  first.click();
  const now = host.querySelector('[role="switch"]') as HTMLButtonElement;
  expect(now.getAttribute('aria-checked')).toBe('true');
  expect(now, 'the same button, not a new one').toBe(first);
  expect(host.querySelectorAll('[role="switch"]').length).toBe(1);
});
