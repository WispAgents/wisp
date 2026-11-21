// Perf audit 2026-11-18 #12: the open task's closed More menu was rebuilt (18 items, 8 separators, 4 submenus, twice)
// by every patch to the task. A closed menu now holds no rows, so a change to its items while closed touches nothing;
// opening draws them, and the keyboard still lands on the first.
import { beforeAll, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { frame, mount, part, tick, until } from './helpers.tsx';
import { Menu, type MenuEntry } from '../src/controls/Menu.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';

beforeAll(() => preloadControls());

const items = (n: number): MenuEntry[] => [
  { value: 'open', label: `Open ${n}`, icon: 'expand' },
  { kind: 'separator' },
  { kind: 'submenu', value: 'sub:copy', label: 'Copy', icon: 'copy', items: [{ value: 'id', label: 'Copy task ID' }, { value: 'resume', label: 'Copy resume command' }] },
  { value: 'delete', label: 'Delete task', danger: true },
];

test('a closed menu draws no rows, and its items changing while closed mutates nothing_menu_closed_no_rows', async () => {
  const [n, setN] = createSignal(0);
  mount(() => <Menu items={items(n())} onSelect={() => {}} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  await tick(20);
  const content = part('task.more');
  expect(content?.getAttribute('data-state')).toBe('closed');
  expect(content?.querySelectorAll('[data-kit="menu-item"]').length).toBe(0);
  expect(part('task.more.flyout')).toBeNull();
  let mutations = 0;
  const mo = new MutationObserver((l) => (mutations += l.length));
  mo.observe(document.body, { subtree: true, childList: true });
  for (let i = 1; i <= 5; i++) setN(i);
  await tick(20);
  mo.disconnect();
  expect(mutations).toBe(0);
});

test('opening draws the rows as they are now, the first one lit_menu_closed_opens_current', async () => {
  const [n, setN] = createSignal(0);
  mount(() => <Menu items={items(n())} onSelect={() => {}} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  setN(7);
  const trigger = part('task.more.trigger');
  trigger?.focus();
  trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
  await until(() => part('task.more')?.getAttribute('data-state') === 'open');
  await frame();
  expect(part('task.more')?.querySelector('[data-value="open"]')?.textContent).toContain('Open 7');
  await until(() => part('task.more')?.querySelector('[data-highlighted]') !== null);
  expect(part('task.more')?.querySelector('[data-highlighted]')?.getAttribute('data-value')).toBe('open');
});
