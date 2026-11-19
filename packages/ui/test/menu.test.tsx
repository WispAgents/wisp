import { afterEach, beforeAll, beforeEach, describe, expect, onTestFinished, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount, part, tick } from './helpers.tsx';
import { ContextMenu, Menu, menuSections, type MenuEntry } from '../src/controls/Menu.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';
import { PlatformLookProvider } from '../src/platform/PlatformLook.tsx';

// the app fetches the overlays' machines once its first screen is up (ADR-0011); a press before that: lazy-controls.test.tsx
beforeAll(() => preloadControls());

const items: MenuEntry[] = [
  { value: 'rerun', label: 'Rerun…', icon: 'rerun' },
  { value: 'done', label: 'Mark done', icon: 'check', keys: 'E' },
  { kind: 'separator' },
  { kind: 'group', id: 'move', label: 'Move', items: [{ value: 'later', label: 'Save for later', hint: 'Keep it in For later' }] },
  { value: 'delete', label: 'Delete task', icon: 'trash', danger: true },
];

const key = (el: Element | null, k: string): void => {
  el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};
const highlighted = (): string | null | undefined => document.querySelector('[data-part="task.more.item"][data-highlighted]')?.getAttribute('data-value');

function open(onSelect = vi.fn()): { trigger: HTMLElement | null; onSelect: typeof onSelect } {
  mount(() => <Menu items={items} onSelect={onSelect} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  const trigger = part('task.more.trigger');
  trigger?.focus();
  return { trigger, onSelect };
}

describe('Menu', () => {
  test('renders its parts: trigger with a name, items, separator, group', async () => {
    const { trigger } = open();
    expect(trigger?.getAttribute('aria-label')).toBe('More');
    expect(trigger?.getAttribute('aria-haspopup')).toBe('menu');
    key(trigger, 'ArrowDown');
    await tick(20);
    expect(part('task.more')?.getAttribute('data-state')).toBe('open');
    expect(document.querySelectorAll('[data-part="task.more.item"]').length).toBe(4);
    expect(part('task.more.separator')).not.toBeNull();
    expect(part('task.more.group')?.textContent).toContain('Move');
    expect(document.querySelector('[data-value="delete"]')?.getAttribute('data-danger')).toBe('');
  });

  test('ArrowDown and ArrowUp move the highlight; Enter selects and closes', async () => {
    const { trigger, onSelect } = open();
    key(trigger, 'ArrowDown');
    await tick(20);
    expect(highlighted()).toBe('rerun');
    const content = part('task.more');
    key(content, 'ArrowDown');
    await tick(10);
    expect(highlighted()).toBe('done');
    key(content, 'ArrowUp');
    await tick(10);
    expect(highlighted()).toBe('rerun');
    key(content, 'ArrowDown');
    await tick(10);
    key(content, 'Enter');
    await tick(20);
    expect(onSelect).toHaveBeenCalledWith('done');
    expect(part('task.more')?.getAttribute('data-state')).toBe('closed');
  });

  test('Escape closes it and focus goes back to the trigger', async () => {
    const { trigger } = open();
    key(trigger, 'ArrowDown');
    await tick(20);
    key(part('task.more'), 'Escape');
    await tick(20);
    expect(part('task.more')?.getAttribute('data-state')).toBe('closed');
    expect(document.activeElement).toBe(trigger);
  });
});

describe('ContextMenu', () => {
  test('a right click on its children opens it', async () => {
    mount(() => (
      <ContextMenu items={items} onSelect={() => {}} part="row.menu">
        <p>row</p>
      </ContextMenu>
    ));
    const area = part('row.menu.area');
    area?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
    await tick(20);
    expect(part('row.menu')?.getAttribute('data-state')).toBe('open');
  });
});

// v1 #786 (web/js/ui/submenu.js): a submenu drills in, its items under a Back row; → opens it, ← and Escape step back.
// That is touch's (and the phone apps') behaviour now: a desktop pointer gets a flyout (owner 2026-11-17,
// menu-flyout.test.tsx), so these run with a touch pointer.
function touchPointer(): void {
  vi.stubGlobal('matchMedia', (q: string) => ({
    media: q,
    matches: q.includes('coarse') || q.includes('hover: none'),
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
}
const nested: MenuEntry[] = [
  { value: 'rename', label: 'Rename', icon: 'pen' },
  {
    kind: 'submenu',
    value: 'sub:copy',
    label: 'Copy',
    icon: 'copy',
    items: [
      { value: 'copy-id', label: 'Copy task ID' },
      { value: 'copy-resume', label: 'Copy resume command' },
    ],
  },
  { value: 'delete', label: 'Delete task', danger: true },
];
const lit = (): string | null | undefined => document.querySelector('[data-part="task.more"] [data-highlighted]')?.getAttribute('data-value');
const shown = (): (string | null)[] => [...document.querySelectorAll('[data-part="task.more"] [data-kit="menu-item"]')].map((e) => e.getAttribute('data-value') ?? e.textContent);

function openNested(onSelect = vi.fn()): { content: HTMLElement | null; onSelect: typeof onSelect } {
  mount(() => <Menu items={nested} onSelect={onSelect} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  const trigger = part('task.more.trigger');
  trigger?.focus();
  key(trigger, 'ArrowDown');
  return { content: part('task.more'), onSelect };
}

describe('Menu submenus', () => {
  beforeEach(touchPointer);
  afterEach(() => {
    vi.unstubAllGlobals();
  });
  test('a submenu row drills in on click: its items under a Back row, the menu stays open_v1_786_submenu_drill', async () => {
    const { onSelect } = openNested();
    await tick(20);
    expect(shown()).toEqual(['rename', 'sub:copy', 'delete']);
    expect(part('task.more.sub')?.getAttribute('aria-haspopup')).toBe('menu');
    part('task.more.sub')?.click();
    await tick(20);
    expect(part('task.more')?.getAttribute('data-state')).toBe('open');
    expect(part('task.more')?.hasAttribute('data-drilled')).toBe(true);
    expect(part('task.more.back')?.textContent).toBe('Copy');
    expect(shown()).toEqual(['menu:back', 'copy-id', 'copy-resume']);
    expect(onSelect).not.toHaveBeenCalled();
    document.querySelector<HTMLElement>('[data-value="copy-resume"]')?.click();
    await tick(20);
    expect(onSelect).toHaveBeenCalledWith('copy-resume');
    expect(part('task.more')?.getAttribute('data-state')).toBe('closed');
  });

  test('→ opens the lit submenu with its first item lit; ← steps back to its row_v1_786_submenu_arrows', async () => {
    const { content } = openNested();
    await tick(20);
    key(content, 'ArrowDown');
    await tick(10);
    expect(lit()).toBe('sub:copy');
    key(content, 'ArrowRight');
    await tick(20);
    expect(part('task.more.back')).not.toBeNull();
    expect(lit()).toBe('copy-id');
    key(content, 'ArrowLeft');
    await tick(20);
    expect(part('task.more.back')).toBeNull();
    expect(lit()).toBe('sub:copy');
    expect(part('task.more')?.getAttribute('data-state')).toBe('open');
  });

  test('Escape in a submenu steps back; a second Escape closes the menu_v1_786_submenu_escape', async () => {
    const { content } = openNested();
    await tick(20);
    key(content, 'ArrowDown');
    await tick(10);
    key(content, 'Enter');
    await tick(20);
    expect(part('task.more.back')).not.toBeNull();
    key(part('task.more'), 'Escape');
    await tick(20);
    expect(part('task.more.back')).toBeNull();
    expect(part('task.more')?.getAttribute('data-state')).toBe('open');
    key(part('task.more'), 'Escape');
    await tick(20);
    expect(part('task.more')?.getAttribute('data-state')).toBe('closed');
  });

  test('Back returns to the menu; reopening starts at the top level_v1_786_submenu_back', async () => {
    openNested();
    await tick(20);
    part('task.more.sub')?.click();
    await tick(20);
    part('task.more.back')?.click();
    await tick(20);
    expect(shown()).toEqual(['rename', 'sub:copy', 'delete']);
    part('task.more.sub')?.click();
    await tick(20);
    key(part('task.more'), 'Escape');
    key(part('task.more'), 'Escape');
    await tick(20);
    const t = part('task.more.trigger');
    t?.focus();
    key(t, 'ArrowDown');
    await tick(20);
    expect(shown()).toEqual(['rename', 'sub:copy', 'delete']);
  });
});

test('a right-click on another row while drilled in opens the menu at its top level_v1_786_submenu_reanchor', async () => {
  touchPointer();
  onTestFinished(() => {
    vi.unstubAllGlobals();
  });
  mount(() => (
    <ContextMenu items={nested} onSelect={() => {}} part="row.menu">
      <p data-part="row.a">a</p>
      <p data-part="row.b">b</p>
    </ContextMenu>
  ));
  const right = (el: Element | null): void => void el?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
  right(part('row.a'));
  await tick(20);
  part('row.menu.sub')?.click();
  await tick(20);
  expect(part('row.menu.back')).not.toBeNull();
  right(part('row.b'));
  await tick(20);
  expect(part('row.menu')?.getAttribute('data-state')).toBe('open');
  expect(part('row.menu.back')).toBeNull();
  expect(part('row.menu')?.hasAttribute('data-drilled')).toBe(false);
});

test('a submenu that folds away while open leaves the top level, and one Escape closes_v1_786_submenu_gone', async () => {
  touchPointer();
  onTestFinished(() => {
    vi.unstubAllGlobals();
  });
  const [items, setItems] = createSignal<MenuEntry[]>(nested);
  mount(() => <Menu items={items()} onSelect={() => {}} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  const trigger = part('task.more.trigger');
  trigger?.focus();
  key(trigger, 'ArrowDown');
  await tick(20);
  part('task.more.sub')?.click();
  await tick(20);
  expect(part('task.more.back')).not.toBeNull();
  // Copy › drops to one item: menuSections shows it inline, no submenu any more
  setItems([nested[0] as MenuEntry, { value: 'copy-id', label: 'Copy task ID' }, nested[2] as MenuEntry]);
  await tick(20);
  expect(part('task.more.back')).toBeNull();
  expect(part('task.more')?.hasAttribute('data-drilled')).toBe(false);
  key(part('task.more'), 'Escape');
  await tick(20);
  expect(part('task.more')?.getAttribute('data-state')).toBe('closed');
});

test('menuSections: a line between groups, empty groups dropped, a submenu of one inline_v1_786_sections', () => {
  const sub = (value: string, items: string[]): MenuEntry => ({ kind: 'submenu', value, label: value, items: items.map((v) => ({ value: v, label: v })) });
  const m = menuSections([[{ value: 'a', label: 'a' }, false], [], [null, sub('s:one', ['only']), sub('s:none', [])], [sub('s:two', ['x', 'y'])]]);
  expect(m.map((e) => ('value' in e ? e.value : e.kind))).toEqual(['a', 'separator', 'only', 'separator', 's:two']);
  expect(menuSections([[sub('s:none', [])], []])).toEqual([]);
});

test('an_item_without_an_icon_keeps_the_icon_column_where_others_have_one_qa_r1_29', async () => {
  // the task's More menu: Lean runs' As in Settings / On / Off started at the section label's edge
  const { trigger } = open();
  key(trigger, 'ArrowDown');
  await tick(20);
  const later = document.querySelector('[data-part="task.more.item"][data-value="later"]');
  expect(later?.firstElementChild?.getAttribute('data-kit')).toBe('menu-icon-gap');
  const rerun = document.querySelector('[data-part="task.more.item"][data-value="rerun"]');
  expect(rerun?.querySelector('[data-kit="menu-icon-gap"]')).toBeNull();
});

test('a_menu_with_no_icons_has_no_empty_icon_column', async () => {
  mount(() => <Menu items={[{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]} onSelect={() => {}} trigger={{ icon: 'dots', 'aria-label': 'Plain' }} part="plain.menu" />);
  key(part('plain.menu.trigger'), 'ArrowDown');
  await tick(20);
  expect(document.querySelector('[data-part="plain.menu.item"] [data-kit="menu-icon-gap"]')).toBeNull();
});

// QA r7 #4: a phone's menu is drawn over the screen (fixed): growing as it drills into a submenu, it never widens the
// page (WebKit kept the frame where the wider menu stood before it moved: the page scrolled sideways by 11 px). The
// browser's stays where Floating UI's default puts it.
describe('a phone menu never widens the page (QA r7 #4)', () => {
  for (const look of ['ios', 'android', 'web'] as const) {
    test(`on_${look}_the_menus_positioner_is_${look === 'web' ? 'absolute' : 'fixed'}`, async () => {
      mount(() => (
        <PlatformLookProvider look={() => look}>
          <Menu items={items} onSelect={vi.fn()} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />
        </PlatformLookProvider>
      ));
      key(part('task.more.trigger'), 'ArrowDown');
      await tick(30);
      const positioner = part('task.more')?.parentElement as HTMLElement;
      expect(positioner.style.position).toBe(look === 'web' ? 'absolute' : 'fixed');
    });
  }
});

// The task head's Task options › Add to crew… did nothing when the crew plugin's later part landed while the submenu
// was open (apps/web task-menu-dialogs, Windows CI run 44379234674): an item that arrives in an open submenu is chosen
// like the others
for (const pointer of ['touch', 'mouse'] as const) {
  test(`an_item_added_to_an_open_submenu_can_be_chosen_${pointer}`, async () => {
    if (pointer === 'touch') touchPointer();
    onTestFinished(() => {
      vi.unstubAllGlobals();
    });
    const onSelect = vi.fn();
    const [items, setItems] = createSignal<MenuEntry[]>(nested);
    mount(() => <Menu items={items()} onSelect={onSelect} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
    const trigger = part('task.more.trigger');
    trigger?.focus();
    key(trigger, 'ArrowDown');
    await tick(20);
    part('task.more.sub')?.click();
    await tick(20);
    const copy = nested[1] as Extract<MenuEntry, { kind: 'submenu' }>;
    const grown: MenuEntry = { ...copy, items: [...copy.items, { value: 'copy-path', label: 'Copy path' }] };
    setItems([nested[0] as MenuEntry, grown, nested[2] as MenuEntry]);
    await tick(20);
    const added = [...document.querySelectorAll<HTMLElement>('[data-value="copy-path"]')].at(-1);
    expect(added, 'the new item shows').toBeDefined();
    added?.click();
    await tick(20);
    expect(onSelect).toHaveBeenCalledWith('copy-path');
  });
}
