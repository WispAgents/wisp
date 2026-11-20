// Owner 2026-11-19 (a phone's browser at 390 px: New task → + → Goal… flew out to the right, off the screen): in the
// browser on a touch-first or narrow screen (a coarse pointer, or under the phone breakpoint) every kit menu opens as a
// bottom sheet, and a submenu drills in place: its items replace the list under a back row (‹ Goal), Back / Escape / ←
// return one level, a tap outside closes. Never a flyout there; a fine pointer on a wide screen keeps its flyouts.
import { afterEach, beforeAll, describe, expect, test, vi } from 'vitest';
import { mount, part, tick, until } from './helpers.tsx';
import { ContextMenu, Menu, SHEET_SCREEN, type MenuEntry } from '../src/controls/Menu.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';
import { phoneOverlays } from '../src/controls/phone-part.ts';

beforeAll(async () => {
  await preloadControls();
  await phoneOverlays.load();
});

type Screen = 'phone-mouse' | 'tablet-touch' | 'desktop';

/** The screen the page reports: narrow with a mouse (a desktop window made small), wide and touch, or a wide desktop. */
function screen(kind: Screen): void {
  const touch = kind === 'tablet-touch';
  const narrow = kind === 'phone-mouse';
  vi.stubGlobal('matchMedia', (q: string) => ({
    media: q,
    matches:
      q === SHEET_SCREEN
        ? touch || narrow
        : q.includes('coarse') || q.includes('hover: none')
          ? touch
          : q.includes('fine') || q.includes('hover: hover')
            ? !touch
            : q.includes('max-width')
              ? narrow
              : false,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

const goals: MenuEntry[] = [
  { value: 'worktree', label: 'Worktree', icon: 'branch' },
  {
    kind: 'submenu',
    value: 'goal',
    label: 'Goal',
    icon: 'target',
    items: [
      { value: 'goal:docs-clean', label: 'docs-clean', hint: 'the docs build without a warning, every link resolves and each new page is in the sidebar', icon: 'target' },
      { value: 'goal:tests-green', label: 'tests-green', hint: 'every test passes', icon: 'target' },
      { value: 'goal:new', label: 'New goal…', icon: 'plus' },
    ],
  },
  { value: 'later', label: 'Start later', icon: 'clock' },
];

const key = (el: Element | null | undefined, k: string): void => {
  el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};
const sheet = (): HTMLElement | null => part('launcher.plus.sheet');
const rows = (): (string | null)[] => [...(part('launcher.plus.list')?.querySelectorAll('[role^="menuitem"]') ?? [])].map((e) => e.getAttribute('data-value') ?? e.textContent);

async function openPlus(onSelect = vi.fn()): Promise<{ onSelect: typeof onSelect; trigger: HTMLElement | null }> {
  mount(() => <Menu items={goals} onSelect={onSelect} trigger={{ icon: 'plus', 'aria-label': 'More options' }} part="launcher.plus" />);
  const trigger = part('launcher.plus.trigger');
  trigger?.click();
  await until(() => part('launcher.plus.list') !== null);
  // the sheet's dialog puts focus on its first row as it opens (a person's first tap comes after that)
  await until(() => document.activeElement?.closest('[data-kit="sheet-menu"]') != null);
  await tick(20);
  return { onSelect, trigger };
}

describe('a sheet screen opens menus as a bottom sheet that drills in', () => {
  for (const kind of ['phone-mouse', 'tablet-touch'] as const) {
    test(`a_submenu_on_a_${kind.replace('-', '_')}_screen_is_a_drill_in_sheet_never_a_flyout`, async () => {
      screen(kind);
      const { onSelect } = await openPlus();
      expect(sheet()?.getAttribute('data-size'), 'the menu sheet').toBe('menu');
      expect(part('launcher.plus.list')?.getAttribute('role')).toBe('menu');
      expect(part('launcher.plus.flyout'), 'no flyout').toBeNull();
      expect(document.querySelector('[data-kit="menu"]'), 'no floating menu').toBeNull();
      expect(rows()).toEqual(['worktree', 'goal', 'later']);
      part('launcher.plus.sub')?.click();
      await tick(20);
      expect(part('launcher.plus.back')?.textContent).toBe('Goal');
      expect(rows()).toEqual([null, 'goal:docs-clean', 'goal:tests-green', 'goal:new'].map((v, i) => (i === 0 ? 'Goal' : v)));
      // a goal's description is its second line (the sheet's CSS wraps it to two lines)
      expect(document.querySelector('[data-value="goal:docs-clean"] [data-kit="sheet-item-hint"]')?.textContent).toContain('every link resolves');
      expect(document.activeElement?.getAttribute('data-value'), 'focus moved into the submenu').toBe('goal:docs-clean');
      expect(onSelect).not.toHaveBeenCalled();
      document.querySelector<HTMLElement>('[data-value="goal:tests-green"]')?.click();
      await tick(20);
      expect(onSelect).toHaveBeenCalledWith('goal:tests-green');
      await until(() => sheet() === null);
    });
  }

  test('the_back_row_returns_one_level_with_focus_on_the_row_that_opened_it', async () => {
    screen('phone-mouse');
    await openPlus();
    part('launcher.plus.sub')?.click();
    await tick(20);
    part('launcher.plus.back')?.click();
    await tick(20);
    expect(part('launcher.plus.back')).toBeNull();
    expect(rows()).toEqual(['worktree', 'goal', 'later']);
    expect(document.activeElement?.getAttribute('data-value')).toBe('goal');
    expect(sheet(), 'still open').not.toBeNull();
  });

  test('escape_steps_back_out_of_a_submenu_and_a_second_escape_closes_the_sheet', async () => {
    screen('tablet-touch');
    const { trigger } = await openPlus();
    part('launcher.plus.sub')?.click();
    await tick(20);
    key(document.activeElement, 'Escape');
    await tick(20);
    expect(part('launcher.plus.back')).toBeNull();
    expect(sheet(), 'one level back, still open').not.toBeNull();
    key(document.activeElement, 'Escape');
    await until(() => sheet() === null);
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');
  });

  test('the_keyboard_moves_drills_and_steps_back', async () => {
    screen('phone-mouse');
    const { onSelect } = await openPlus();
    expect(document.activeElement?.getAttribute('data-value'), 'focus goes into the sheet').toBe('worktree');
    key(document.activeElement, 'ArrowDown');
    expect(document.activeElement?.getAttribute('data-value')).toBe('goal');
    key(document.activeElement, 'ArrowRight');
    await tick(20);
    expect(document.activeElement?.getAttribute('data-value')).toBe('goal:docs-clean');
    key(document.activeElement, 'End');
    expect(document.activeElement?.getAttribute('data-value')).toBe('goal:new');
    key(document.activeElement, 'ArrowLeft');
    await tick(20);
    expect(document.activeElement?.getAttribute('data-value')).toBe('goal');
    key(document.activeElement, 'ArrowUp');
    (document.activeElement as HTMLElement | null)?.click();
    await tick(20);
    expect(onSelect).toHaveBeenCalledWith('worktree');
  });

  test('a_right_click_or_long_press_on_a_row_opens_its_sheet_titled_by_the_row', async () => {
    screen('tablet-touch');
    const onSelect = vi.fn();
    mount(() => (
      <ContextMenu items={goals} onSelect={onSelect} part="row.menu">
        <div data-kit="list-row">
          <span data-kit="list-row-title">Fix the login</span>
        </div>
      </ContextMenu>
    ));
    document.querySelector('[data-kit="list-row-title"]')?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
    await until(() => part('row.menu.sheet') !== null);
    expect(part('row.menu.sheet')?.textContent).toContain('Fix the login');
    expect(part('row.menu.flyout')).toBeNull();
    part('row.menu.sub')?.click();
    await tick(20);
    expect(part('row.menu.back')?.textContent).toBe('Goal');
  });
});

test('a_wide_desktop_keeps_the_floating_menu_and_its_flyout', async () => {
  screen('desktop');
  mount(() => <Menu items={goals} onSelect={vi.fn()} trigger={{ icon: 'plus', 'aria-label': 'More options' }} part="launcher.plus" />);
  const trigger = part('launcher.plus.trigger');
  trigger?.focus();
  key(trigger, 'ArrowDown');
  await until(() => part('launcher.plus')?.getAttribute('data-state') === 'open');
  expect(document.querySelector('[data-kit="menu"][data-part="launcher.plus"]')).not.toBeNull();
  expect(sheet()).toBeNull();
  part('launcher.plus.sub')?.click();
  await until(() => part('launcher.plus.flyout')?.getAttribute('data-state') === 'open');
  expect(part('launcher.plus.back'), 'no back row on the desktop').toBeNull();
});
