// Owner 2026-11-17: on a desktop pointer (hovering, fine; the browser, not the phone apps) a submenu is a flyout beside
// its row, the standard cascading menu (Zag's nested menus): hover opens it at once (v1's opened at the press; perf r2
// #10: Zag's 200 ms intent read as slow), leaving closes
// it, → / Enter / click open it, ← and Escape close only it and light its row again, choosing an item closes the whole
// menu. Touch and the phone apps keep v1 #786's drill-in (menu.test.tsx 'Menu submenus').
import { afterEach, beforeAll, describe, expect, test, vi } from 'vitest';
import { frame, mount, part, tick, until } from './helpers.tsx';
import { ContextMenu, Menu, type MenuEntry } from '../src/controls/Menu.tsx';
import { Dialog } from '../src/controls/Dialog.tsx';
import { createSignal } from 'solid-js';
import { preloadControls } from '../src/controls/lazy-part.ts';

beforeAll(() => preloadControls());

/** The pointer the page reports: a desktop mouse (hover, fine) or a touch screen (none, coarse). */
function pointer(kind: 'mouse' | 'touch'): void {
  const mouse = kind === 'mouse';
  vi.stubGlobal('matchMedia', (q: string) => ({
    media: q,
    matches: q.includes('coarse') || q.includes('hover: none') ? !mouse : q.includes('fine') || q.includes('hover: hover') ? mouse : false,
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
  vi.restoreAllMocks();
  document.documentElement.removeAttribute('data-platform');
});

const items: MenuEntry[] = [
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

const key = (el: Element | null, k: string): void => {
  el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};
const mouse = (el: Element | null, type: 'pointermove' | 'pointerleave' | 'pointerenter', x = 10, y = 10): void => {
  el?.dispatchEvent(new PointerEvent(type, { pointerType: 'mouse', bubbles: type === 'pointermove', clientX: x, clientY: y }));
};
const state = (name: string): string | null | undefined => part(name)?.getAttribute('data-state');
const row = (): HTMLElement | null => part('task.more.sub');
/** the value lit in the top level (a submenu's row is lit by its trigger id: Zag's trigger item) */
const litTop = (): string | null | undefined => part('task.more')?.querySelector(':scope [data-highlighted]')?.getAttribute('data-value');
const litFly = (): string | null | undefined => part('task.more.flyout')?.querySelector('[data-highlighted]')?.getAttribute('data-value');

/** `ok` within a few tasks, however slow the clock: what opens "at once" never waits on a timer. */
async function soon(ok: () => boolean): Promise<void> {
  // the count is the point: "at once" can't wait on the clock, and 5 hops (78 ms on Windows' 15.6 ms ticks) are still
  // under the 200 ms hover intent this replaced
  // check:allow timer-loop-in-test a fixed count proves the open never waits on a timer
  for (let i = 0; i < 5 && !ok(); i++) await tick();
}

async function openMenu(onSelect = vi.fn()): Promise<{ content: HTMLElement | null; onSelect: typeof onSelect; trigger: HTMLElement | null }> {
  mount(() => <Menu items={items} onSelect={onSelect} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
  const trigger = part('task.more.trigger');
  trigger?.focus();
  key(trigger, 'ArrowDown');
  await until(() => state('task.more') === 'open');
  await frame();
  return { content: part('task.more'), onSelect, trigger };
}

describe('desktop: a submenu is a flyout beside its row', () => {
  test('the row stays in the menu, names its flyout, and no Back row is drawn_flyout_shape', async () => {
    pointer('mouse');
    await openMenu();
    expect(row()?.getAttribute('aria-haspopup')).toBe('menu');
    expect(row()?.getAttribute('data-submenu')).toBe('sub:copy');
    expect(part('task.more.back')).toBeNull();
    expect(part('task.more')?.hasAttribute('data-drilled')).toBe(false);
    // the flyout is its own floating list, closed until asked
    expect(state('task.more.flyout')).toBe('closed');
    expect(part('task.more.flyout')?.getAttribute('data-flyout')).toBe('');
  });

  test('hovering the row opens the flyout at once; leaving closes it_flyout_hover', async () => {
    pointer('mouse');
    await openMenu();
    mouse(row(), 'pointermove');
    // at once: a few tasks, where Zag's own hover intent waits a 200 ms timer
    await soon(() => state('task.more.flyout') === 'open');
    expect(state('task.more.flyout')).toBe('open');
    expect(row()?.getAttribute('aria-expanded')).toBe('true');
    expect([...(part('task.more.flyout')?.querySelectorAll('[data-kit="menu-item"]') ?? [])].map((e) => e.getAttribute('data-value'))).toEqual(['copy-id', 'copy-resume']);
    // the top level stays as it was
    expect(state('task.more')).toBe('open');
    // leave the row for elsewhere (not into the flyout): it closes
    mouse(row(), 'pointerleave', 10, 10);
    mouse(document.body, 'pointermove', 900, 900);
    await until(() => state('task.more.flyout') === 'closed', 1000);
    expect(state('task.more.flyout')).toBe('closed');
    expect(state('task.more')).toBe('open');
  });

  test('hovering another submenu row with one flyout open swaps them at once_flyout_sibling', async () => {
    pointer('mouse');
    const two: MenuEntry[] = [
      ...items,
      { kind: 'submenu', value: 'sub:open', label: 'Open in', part: 'task.more.open', items: [{ value: 'finder', label: 'Finder' }, { value: 'editor', label: 'Editor' }] },
    ];
    mount(() => <Menu items={two} onSelect={() => {}} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
    const trigger = part('task.more.trigger');
    trigger?.focus();
    key(trigger, 'ArrowDown');
    await until(() => state('task.more') === 'open');
    await frame();
    const flies = (): (string | null)[] => [...document.querySelectorAll('[data-flyout]')].map((f) => f.getAttribute('data-state'));
    mouse(row(), 'pointermove');
    await soon(() => flies().join() === 'open,closed');
    expect(flies()).toEqual(['open', 'closed']);
    // straight down onto the other row (not toward the open flyout): it opens and the first closes, both at once
    mouse(row(), 'pointerleave', 10, 132);
    mouse(part('task.more.open'), 'pointermove', 10, 200);
    await soon(() => flies().join() === 'closed,open');
    expect(flies()).toEqual(['closed', 'open']);
    expect(state('task.more')).toBe('open');
  });

  test('a diagonal into the flyout crosses the rows between and the flyout stays open (the safe triangle)_flyout_diagonal', async () => {
    pointer('mouse');
    await openMenu();
    mouse(row(), 'pointermove', 100, 116);
    await until(() => state('task.more.flyout') === 'open', 1000);
    // lay the two out: the row at 0–200 × 100–132, its flyout beside it at 200–400 × 96–300
    const r = { x: 0, y: 100, left: 0, top: 100, right: 200, bottom: 132, width: 200, height: 32 };
    const f = { x: 200, y: 96, left: 200, top: 96, right: 400, bottom: 300, width: 200, height: 204 };
    vi.spyOn(row() as HTMLElement, 'getBoundingClientRect').mockReturnValue({ ...r, toJSON: () => r } as DOMRect);
    vi.spyOn(part('task.more.flyout') as HTMLElement, 'getBoundingClientRect').mockReturnValue({ ...f, toJSON: () => f } as DOMRect);
    mouse(row(), 'pointerleave', 150, 132);
    // slowly, past Zag's 100 ms, down and right over the Delete row toward the flyout's lower items
    const below = part('task.more')?.querySelector('[data-value="delete"]') ?? document.body;
    for (let i = 1; i <= 6; i++) {
      mouse(below, 'pointermove', 150 + i * 8, 132 + i * 12);
      await tick(40);
      expect(state('task.more.flyout'), `step ${i}`).toBe('open');
    }
    // the rows it crossed didn't light
    expect(part('task.more')?.querySelector('[data-value="delete"]')?.hasAttribute('data-highlighted')).toBe(false);
    // off the path, straight down: it closes
    mouse(below, 'pointermove', 150, 260);
    await until(() => state('task.more.flyout') === 'closed', 500);
    expect(state('task.more.flyout')).toBe('closed');
    expect(state('task.more')).toBe('open');
  });

  test('resting inside the triangle without reaching the flyout closes it after a grace_flyout_grace', async () => {
    pointer('mouse');
    await openMenu();
    mouse(row(), 'pointermove', 100, 116);
    await until(() => state('task.more.flyout') === 'open', 1000);
    const r = { x: 0, y: 100, left: 0, top: 100, right: 200, bottom: 132, width: 200, height: 32 };
    const f = { x: 200, y: 96, left: 200, top: 96, right: 400, bottom: 300, width: 200, height: 204 };
    vi.spyOn(row() as HTMLElement, 'getBoundingClientRect').mockReturnValue({ ...r, toJSON: () => r } as DOMRect);
    vi.spyOn(part('task.more.flyout') as HTMLElement, 'getBoundingClientRect').mockReturnValue({ ...f, toJSON: () => f } as DOMRect);
    mouse(row(), 'pointerleave', 150, 132);
    mouse(document.body, 'pointermove', 170, 150);
    await tick(150);
    expect(state('task.more.flyout')).toBe('open');
    await until(() => state('task.more.flyout') === 'closed', 1000);
    expect(state('task.more.flyout')).toBe('closed');
  });

  test('a click on the row opens the flyout at once_flyout_click', async () => {
    pointer('mouse');
    await openMenu();
    row()?.click();
    await until(() => state('task.more.flyout') === 'open', 500);
    expect(state('task.more.flyout')).toBe('open');
  });

});

describe('desktop: the flyout from the keyboard, and choosing in it', () => {
  test('→ opens the lit row’s flyout with its first item lit; ← closes only it and lights its row_flyout_arrows', async () => {
    pointer('mouse');
    const { content } = await openMenu();
    key(content, 'ArrowDown');
    await tick(10);
    expect(litTop()).toBe(row()?.getAttribute('data-value'));
    key(content, 'ArrowRight');
    await until(() => state('task.more.flyout') === 'open', 500);
    await until(() => litFly() === 'copy-id', 500);
    expect(litFly()).toBe('copy-id');
    key(part('task.more.flyout'), 'ArrowLeft');
    await until(() => state('task.more.flyout') === 'closed', 500);
    expect(state('task.more')).toBe('open');
    expect(litTop()).toBe(row()?.getAttribute('data-value'));
  });

  test('Enter on the row opens the flyout; Escape closes only the flyout, a second Escape the menu_flyout_escape', async () => {
    pointer('mouse');
    const { content, trigger } = await openMenu();
    key(content, 'ArrowDown');
    await tick(10);
    key(content, 'Enter');
    await until(() => state('task.more.flyout') === 'open', 500);
    expect(state('task.more.flyout')).toBe('open');
    await frame();
    key(document.activeElement, 'Escape');
    await until(() => state('task.more.flyout') === 'closed', 500);
    expect(state('task.more')).toBe('open');
    expect(litTop()).toBe(row()?.getAttribute('data-value'));
    await frame();
    key(document.activeElement, 'Escape');
    await until(() => state('task.more') === 'closed', 500);
    expect(state('task.more')).toBe('closed');
    await until(() => document.activeElement === trigger, 500);
    expect(document.activeElement).toBe(trigger);
  });

  test('choosing an item in the flyout runs it and closes the whole menu_flyout_choose', async () => {
    pointer('mouse');
    const { onSelect } = await openMenu();
    row()?.click();
    await until(() => state('task.more.flyout') === 'open', 500);
    part('task.more.flyout')?.querySelector<HTMLElement>('[data-value="copy-resume"]')?.click();
    await until(() => state('task.more') === 'closed', 500);
    // chosen once the press has ended (its click done): see the next test
    await until(() => onSelect.mock.calls.length > 0, 500);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith('copy-resume');
    // the flyout went with its menu: a closed menu draws no rows (perf audit 2026-11-18 #12)
    expect(part('task.more.flyout')).toBeNull();
    expect(state('task.more')).toBe('closed');
  });

  test('a right-click menu (the task row’s) flies out too, and choosing closes it_flyout_context_menu', async () => {
    pointer('mouse');
    const onSelect = vi.fn();
    mount(() => (
      <ContextMenu items={items} onSelect={onSelect} part="row.menu">
        <p data-part="row.a">a</p>
      </ContextMenu>
    ));
    part('row.a')?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
    await until(() => state('row.menu') === 'open');
    mouse(part('row.menu.sub'), 'pointermove');
    await until(() => state('row.menu.flyout') === 'open', 1000);
    expect(state('row.menu.flyout')).toBe('open');
    part('row.menu.flyout')?.querySelector<HTMLElement>('[data-value="copy-id"]')?.click();
    await until(() => state('row.menu') === 'closed', 500);
    await until(() => onSelect.mock.calls.length > 0, 500);
    expect(onSelect).toHaveBeenCalledWith('copy-id');
  });

  // QA r14 #4: + › Recipes › New opened the recipe dialog and the same press closed it again (its click, the menu
  // handing focus back to its button: both outside the dialog): nothing seemed to happen. What a flyout's item does
  // runs once the press and the menu's close are over, so a dialog it opens stays
  test('a dialog a flyout item opens stays open_flyout_choose_opens', async () => {
    pointer('mouse');
    const [open, setOpen] = createSignal(false);
    const closes: boolean[] = [];
    mount(() => (
      <>
        <Menu items={items} onSelect={() => setOpen(true)} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />
        <Dialog title="New recipe" open={open()} onOpenChange={(o) => (closes.push(o), setOpen(o))} part="t.dialog">
          <input data-part="t.dialog.field" />
        </Dialog>
      </>
    ));
    const trigger = part('task.more.trigger');
    trigger?.focus();
    key(trigger, 'ArrowDown');
    await until(() => state('task.more') === 'open');
    await frame();
    row()?.click();
    await until(() => state('task.more.flyout') === 'open', 500);
    const item = part('task.more.flyout')?.querySelector<HTMLElement>('[data-value="copy-id"]');
    for (const type of ['pointerdown', 'pointerup'] as const) item?.dispatchEvent(new PointerEvent(type, { pointerType: 'mouse', bubbles: true, button: 0 }));
    item?.click();
    // not within the press's own task (its microtasks included): a browser's click and the menu's focus hand-back
    // come there, and would close what the item opened
    await Promise.resolve();
    await Promise.resolve();
    expect(open(), 'not yet: the press is still going').toBe(false);
    await until(() => open(), 500);
    await until(() => state('task.more') === 'closed', 500);
    await tick(100);
    expect(closes, 'nothing closed it').not.toContain(false);
    expect(open()).toBe(true);
    expect(part('t.dialog.field'), 'the dialog is drawn').not.toBeNull();
  });
});

describe('touch and the phone apps keep the drill-in', () => {
  test('a touch pointer drills in under a Back row, no flyout_flyout_touch_drills', async () => {
    pointer('touch');
    await openMenu();
    expect(part('task.more.flyout')).toBeNull();
    row()?.click();
    await tick(20);
    expect(part('task.more.back')?.textContent).toBe('Copy');
    expect(part('task.more')?.hasAttribute('data-drilled')).toBe(true);
  });

  test('the phone apps drill in even with a mouse attached_flyout_phone_drills', async () => {
    pointer('mouse');
    document.documentElement.setAttribute('data-platform', 'android');
    await openMenu();
    expect(part('task.more.flyout')).toBeNull();
    row()?.click();
    await tick(20);
    expect(part('task.more.back')).not.toBeNull();
  });
});
