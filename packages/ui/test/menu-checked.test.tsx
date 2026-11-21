// QA round 6 desktop #6: Transcript in the task's ⋯ drew its check, but said nothing to assistive tech: a plain
// `menuitem`. A toggle (an item with `checked`) is a `menuitemcheckbox` with `aria-checked`, as the phone menu has it.
import { beforeAll, expect, test } from 'vitest';
import { mount, part, tick } from './helpers.tsx';
import { Menu, type MenuEntry } from '../src/controls/Menu.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';

beforeAll(() => preloadControls());

const items: MenuEntry[] = [
  { value: 'transcript', label: 'Transcript', keys: 'V', checked: true },
  { value: 'shell', label: 'Shell', checked: false },
  { value: 'rerun', label: 'Rerun…' },
];

test('a_toggle_in_a_menu_is_a_menuitemcheckbox_with_its_state_qa_r6_6', async () => {
  mount(() => <Menu items={items} onSelect={() => {}} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  const trigger = part('task.more.trigger');
  trigger?.focus();
  trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  await tick(20);
  const at = (v: string): Element | null => document.querySelector(`[data-kit="menu-item"][data-value="${v}"]`);
  expect([at('transcript')?.getAttribute('role'), at('transcript')?.getAttribute('aria-checked')]).toEqual(['menuitemcheckbox', 'true']);
  expect([at('shell')?.getAttribute('role'), at('shell')?.getAttribute('aria-checked')]).toEqual(['menuitemcheckbox', 'false']);
  expect([at('rerun')?.getAttribute('role'), at('rerun')?.hasAttribute('aria-checked')]).toEqual(['menuitem', false]);
});
