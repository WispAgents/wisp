// A right-click before the menu's machine has loaded (ADR-0011, controls/lazy-part.ts): the browser's own menu doesn't
// open, and the menu opens at the pointer once the machine lands (its own file: a machine loads once per page).
import { expect, test, vi } from 'vitest';
import { mount, part, tick } from './helpers.tsx';
import { ContextMenu } from '../src/controls/Menu.tsx';

test('ContextMenu: a right-click before the machine opens it at the pointer as it lands', async () => {
  mount(() => (
    <ContextMenu items={[{ value: 'rename', label: 'Rename' }]} onSelect={vi.fn()} part="row.menu">
      <span>a task</span>
    </ContextMenu>
  ));
  const area = part('row.menu.area');
  const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 40, clientY: 60 });
  area?.dispatchEvent(e);
  expect(e.defaultPrevented).toBe(true);
  await import('../src/controls/menu-machine.tsx');
  await tick(10);
  expect(part('row.menu')?.getAttribute('data-state')).toBe('open');
  expect(document.querySelectorAll('[data-part="row.menu.item"]').length).toBe(1);
});
