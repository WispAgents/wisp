// Owner-list #63 (owner 2026-11-19, "what if a user has a lot of goals, what will happen in that submenu?"): a submenu
// that grows with the formula (Goal…, Recipes) has a filter box over its rows. Typing narrows them by name and second
// line, any case, each word; Enter picks the first match; ↓ goes to the rows; the rows it keeps (New goal…) stay under the
// matches; nothing matching says so. On a desktop pointer the flyout's box takes the focus as it opens; on a sheet
// screen the drilled-in sheet has it over its rows.
import { afterEach, beforeAll, describe, expect, test, vi } from 'vitest';
import { frame, mount, part, tick, until } from './helpers.tsx';
import { Menu, SHEET_SCREEN, type MenuEntry, type MenuItem } from '../src/controls/Menu.tsx';
import type { MenuSubmenu } from '../src/controls/menu-entries.ts';
import { filteredItems, firstMatch, matchesQuery, NO_MATCH } from '../src/controls/menu-filtering.ts';
import { preloadControls } from '../src/controls/lazy-part.ts';
import { phoneOverlays } from '../src/controls/phone-part.ts';

beforeAll(async () => {
  await preloadControls();
  await phoneOverlays.load();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

/** a desktop mouse, or a narrow window with a mouse (a sheet screen) */
function screen(kind: 'desktop' | 'phone-mouse'): void {
  const narrow = kind === 'phone-mouse';
  vi.stubGlobal('matchMedia', (q: string) => ({
    media: q,
    matches: q === SHEET_SCREEN ? narrow : q.includes('coarse') || q.includes('hover: none') ? false : q.includes('fine') || q.includes('hover: hover') ? true : q.includes('max-width') ? narrow : false,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
}

const goal = (name: string, hint: string): MenuItem => ({ value: `goal:${name}`, label: name, hint, icon: 'target' });
const all: MenuItem[] = [
  goal('green-build', 'npm test passes'),
  goal('docs-clean', 'the docs build without a warning'),
  goal('lint-zero', 'eslint reports nothing'),
  goal('perf-budget', 'the first screen stays under its ratchet'),
  goal('a11y', 'axe finds no violation'),
  goal('types-strict', 'tsc --strict passes'),
  goal('deploy-ok', 'the preview deploy answers 200'),
  goal('mobile-fit', 'nothing wraps at 320 px'),
  goal('release-notes', 'the changelog names every change'),
];
const sub: MenuSubmenu = {
  kind: 'submenu',
  value: 'goal',
  label: 'Goal…',
  icon: 'target',
  items: [...all.slice(0, 7), { value: 'goal:all', label: 'All goals (9)…', icon: 'search' }, { value: 'goal:new', label: 'New goal…', icon: 'plus' }],
  filter: { label: 'Filter goals', placeholder: 'Filter by name or what done means', empty: 'No goal matches', all, keep: ['goal:all', 'goal:new'] },
};
const values = (items: MenuSubmenu['items']): string[] => items.map((e) => (e.kind === 'separator' ? '-' : e.kind === 'group' ? `g:${e.id}` : e.value));

describe('the filter, pure', () => {
  test('each_word_typed_matches_the_label_or_the_second_line_any_case', () => {
    expect(matchesQuery({ label: 'green-build', hint: 'npm test passes' }, 'GREEN')).toBe(true);
    expect(matchesQuery({ label: 'green-build', hint: 'npm test passes' }, 'test green')).toBe(true);
    expect(matchesQuery({ label: 'green-build', hint: 'npm test passes' }, 'test lint')).toBe(false);
    expect(matchesQuery({ label: 'green-build' }, '  ')).toBe(true);
  });

  test('typing_searches_every_row_not_only_the_ones_shown_and_keeps_new_goal_under_the_matches', () => {
    expect(filteredItems(sub, ''), 'nothing typed: its own rows').toBe(sub.items);
    // release-notes is past the first seven: the filter finds it all the same
    expect(values(filteredItems(sub, 'changelog'))).toEqual(['goal:release-notes', '-', 'goal:all', 'goal:new']);
    expect(values(filteredItems(sub, 'passes'))).toEqual(['goal:green-build', 'goal:types-strict', '-', 'goal:all', 'goal:new']);
    expect(firstMatch(filteredItems(sub, 'passes'))).toBe('goal:green-build');
  });

  test('nothing_matching_shows_one_row_that_says_so_and_enter_picks_nothing_from_it', () => {
    const none = filteredItems(sub, 'zzz');
    expect(values(none)).toEqual([NO_MATCH, '-', 'goal:all', 'goal:new']);
    expect(none[0]).toMatchObject({ label: 'No goal matches', disabled: true });
    // Enter then takes the first row it can: All goals… (where a longer search can go on)
    expect(firstMatch(none)).toBe('goal:all');
    expect(firstMatch(filteredItems({ ...sub, filter: { ...sub.filter!, keep: [] } }, 'zzz'))).toBeUndefined();
  });
});

const items: MenuEntry[] = [{ value: 'attach', label: 'Attach', icon: 'clip' }, sub];

function type(el: Element | null | undefined, text: string): void {
  const input = el as HTMLInputElement;
  input.value = text;
  input.dispatchEvent(new InputEvent('input', { bubbles: true }));
}
const press = (el: Element | null | undefined, k: string): boolean => !!el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));

describe('desktop: the flyout', () => {
  async function openFlyout(onSelect = vi.fn()): Promise<typeof onSelect> {
    screen('desktop');
    mount(() => <Menu items={items} onSelect={onSelect} trigger={{ icon: 'plus', 'aria-label': 'More options' }} part="launcher.plus" />);
    const trigger = part('launcher.plus.trigger');
    trigger?.focus();
    press(trigger, 'ArrowDown');
    await until(() => part('launcher.plus')?.getAttribute('data-state') === 'open');
    part('launcher.plus.sub')?.click();
    await until(() => part('launcher.plus.flyout')?.getAttribute('data-state') === 'open', 500);
    await frame();
    await frame();
    return onSelect;
  }
  const flyRows = (): (string | null)[] => [...(part('launcher.plus.flyout')?.querySelectorAll('[data-kit="menu-item"]') ?? [])].map((e) => e.getAttribute('data-value'));

  test('the_filter_box_has_the_focus_as_the_flyout_opens_and_typing_narrows_the_rows', async () => {
    await openFlyout();
    const box = part('launcher.plus.filter')?.querySelector('input');
    expect(box?.getAttribute('aria-label')).toBe('Filter goals');
    expect(document.activeElement, 'typing narrows it at once').toBe(box);
    expect(flyRows()).toEqual([...all.slice(0, 7).map((g) => g.value), 'goal:all', 'goal:new']);
    type(box, 'eslint');
    await tick();
    expect(flyRows()).toEqual(['goal:lint-zero', 'goal:all', 'goal:new']);
    // a letter is the box's: the menu's typeahead never moved the highlight or closed anything
    expect(press(box, 'l')).toBe(true);
    expect(part('launcher.plus.flyout')?.getAttribute('data-state')).toBe('open');
  });

  test('enter_in_the_box_picks_the_first_match_and_closes_the_whole_menu', async () => {
    const onSelect = await openFlyout();
    const box = part('launcher.plus.filter')?.querySelector('input');
    type(box, 'changelog');
    await tick();
    press(box, 'Enter');
    await until(() => onSelect.mock.calls.length > 0, 500);
    expect(onSelect).toHaveBeenCalledWith('goal:release-notes');
    await until(() => part('launcher.plus')?.getAttribute('data-state') === 'closed', 500);
  });

  test('arrow_down_from_the_box_lights_the_first_match', async () => {
    await openFlyout();
    const box = part('launcher.plus.filter')?.querySelector('input');
    type(box, 'passes');
    await tick();
    press(box, 'ArrowDown');
    await until(() => part('launcher.plus.flyout')?.querySelector('[data-highlighted]') != null, 500);
    expect(part('launcher.plus.flyout')?.querySelector('[data-highlighted]')?.getAttribute('data-value')).toBe('goal:green-build');
  });
});

describe('a sheet screen: the drilled-in sheet', () => {
  test('the_sheets_submenu_has_the_filter_over_its_rows_and_enter_picks_the_first_match', async () => {
    screen('phone-mouse');
    const onSelect = vi.fn();
    mount(() => <Menu items={items} onSelect={onSelect} trigger={{ icon: 'plus', 'aria-label': 'More options' }} part="launcher.plus" />);
    part('launcher.plus.trigger')?.click();
    await until(() => part('launcher.plus.list') !== null);
    await tick(20);
    part('launcher.plus.sub')?.click();
    await tick(20);
    const box = part('launcher.plus.filter')?.querySelector('input');
    expect(box, 'the box, over the rows').not.toBeNull();
    expect(document.activeElement, 'a keyboard’s screen: the box has the focus').toBe(box);
    type(box, 'eslint');
    await tick();
    const shown = [...(part('launcher.plus.list')?.querySelectorAll('[data-kit="sheet-item"]:not([data-back])') ?? [])].map((e) => e.getAttribute('data-value'));
    expect(shown).toEqual(['goal:lint-zero', 'goal:all', 'goal:new']);
    press(box, 'Enter');
    await tick(20);
    expect(onSelect).toHaveBeenCalledWith('goal:lint-zero');
  });

  test('escape_in_the_sheets_filter_steps_back_one_level', async () => {
    screen('phone-mouse');
    mount(() => <Menu items={items} onSelect={vi.fn()} trigger={{ icon: 'plus', 'aria-label': 'More options' }} part="launcher.plus" />);
    part('launcher.plus.trigger')?.click();
    await until(() => part('launcher.plus.list') !== null);
    await tick(20);
    part('launcher.plus.sub')?.click();
    await tick(20);
    press(part('launcher.plus.filter')?.querySelector('input'), 'Escape');
    await tick(20);
    expect(part('launcher.plus.back'), 'back at the top').toBeNull();
    expect(part('launcher.plus.filter')).toBeNull();
    expect(part('launcher.plus.sheet'), 'still open').not.toBeNull();
  });
});
