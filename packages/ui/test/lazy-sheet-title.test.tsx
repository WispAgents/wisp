// Audit G tasks #1: in the browser on a sheet screen (390 px), the first ⋯ after a page load opened a sheet titled with
// the list's top row ("Filler 12") over the tapped row's actions. The press came before the sheet's machine had loaded,
// and was replayed on the whole list, whose first title the sheet read. It is replayed on what was pressed, so the sheet
// names the tapped row (its own file: the machine must not be loaded before the press).
import { afterEach, expect, test, vi } from 'vitest';
import { mount, part, until } from './helpers.tsx';
import { ContextMenu, SHEET_SCREEN } from '../src/controls/Menu.tsx';

afterEach(() => {
  vi.unstubAllGlobals();
});

test('the_first_sheet_after_a_load_is_titled_by_the_pressed_row_not_the_lists_first_audit_g_tasks_1', async () => {
  vi.stubGlobal('matchMedia', (q: string) => ({
    media: q,
    matches: q === SHEET_SCREEN || q.includes('max-width'),
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
  mount(() => (
    <ContextMenu items={[{ value: 'pin', label: 'Pin' }]} onSelect={vi.fn()} part="row.menu">
      <div data-kit="list-row">
        <span data-kit="list-row-title">Filler 12</span>
      </div>
      <div data-kit="list-row">
        <span data-kit="list-row-title">gate ask1</span>
        <button type="button" data-part="row.more">
          ⋯
        </button>
      </div>
    </ContextMenu>
  ));
  // the row's ⋯ asks for the row's menu as a right-click on itself does
  part('row.more')?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 300, clientY: 80 }));
  await until(() => part('row.menu.sheet') !== null);
  const sheet = part('row.menu.sheet');
  expect(sheet?.textContent).toContain('gate ask1');
  expect(sheet?.textContent).not.toContain('Filler 12');
});
