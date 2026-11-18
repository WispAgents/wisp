/// <reference types="vite/client" />
// ADR-0065 build step 5: the overlays' phone variants (design/mobile.md §A.4–§A.7). Under `data-platform` (here a
// PlatformLookProvider) Dialog, Sheet, ContextMenu, Popover, Tooltip and the toasts draw their platform's shape; the
// browser's render, portals included, is byte for byte what it was before the variants landed
// (test/goldens/platform-browser-overlays.json), and the browser never fetches the phone halves (controls/phone.tsx).
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createSignal, type JSX } from 'solid-js';
import { render } from 'solid-js/web';
import goldenRaw from './goldens/platform-browser-overlays.json?raw';
import { mount, tick } from './helpers.tsx';
import { MENU_ITEMS, overlayScenes, type OverlayScene } from './platform-scenes-overlays.tsx';
import {
  Button,
  ContextMenu,
  Dialog,
  Fab,
  GestureHostProvider,
  ListRow,
  NavigationBar,
  PlatformLookProvider,
  Popover,
  Sheet,
  Swipe,
  Text,
  Tooltip,
  preloadControls,
  type GestureAction,
  type GestureHost,
  type Haptic,
  type Look,
} from '../src/index.ts';
import { phoneOverlays } from '../src/controls/phone-part.ts';
import { CONTEXT_MARGIN, contextPlace, detentAfter, detentsOf, phoneRows, rowsHeight, SELECT_VALUE, tipPlace, TIP_MS } from '../src/controls/phone-math.ts';
import { markRoot } from '../src/platform/root-mark.ts';

const GOLDEN = JSON.parse(goldenRaw) as Record<string, string>;

// The browser halves of the lazy controls, loaded as the file loads: their first import is transformed in the same
// process as every other package's (CI's Windows suite: one run, 800 s of transforms), which no test should time.
// Inside the test it took the browser-look render past its 5 s on Windows (CI 1b971518). The phone halves stay
// unloaded: `the_browser_never_fetches_the_phone_halves_not_even_on_idle` checks that.
await preloadControls();
const noop = (): void => {};
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const qa = (sel: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(sel)];

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

interface FakeHost extends GestureHost {
  readonly ticks: Haptic[];
  readonly given: Map<Element, readonly GestureAction[]>;
}
function fakeHost(): FakeHost {
  const ticks: Haptic[] = [];
  const given = new Map<Element, readonly GestureAction[]>();
  return { ticks, given, haptic: (k) => void ticks.push(k), actions: (el, list) => (given.set(el, list), () => given.delete(el)) };
}
const on = (look: Look, view: () => JSX.Element, h: GestureHost = fakeHost()): HTMLElement =>
  mount(() => (
    <PlatformLookProvider look={() => look}>
      <GestureHostProvider host={h}>{view()}</GestureHostProvider>
    </PlatformLookProvider>
  ));
/** the phone chunk and the dialog machine have landed, and what they bind has run */
const landed = async (): Promise<void> => {
  await Promise.all([phoneOverlays.load(), preloadControls()]);
  await tick(20);
};
const contextAt = (el: Element | null, x = 20): void => {
  el?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: x, clientY: 10 }));
};
function pe(el: Element, type: string, x: number, y = 100): void {
  el.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, pointerType: 'touch', isPrimary: true, pointerId: 1 } as PointerEventInit));
}
/** A touch event at (x, y): happy-dom's TouchEvent without Touch objects, so the list is set by hand. */
function te(el: Element, type: string, x: number, y: number, t = 0): void {
  const e = new Event(type, { bubbles: true, cancelable: true });
  const touches = type === 'touchend' || type === 'touchcancel' ? [] : [{ clientX: x, clientY: y }];
  Object.defineProperties(e, { touches: { value: touches }, changedTouches: { value: [{ clientX: x, clientY: y }] }, timeStamp: { value: t } });
  el.dispatchEvent(e);
}
const dragDown = (el: Element, from: number, to: number, t0 = 0): void => {
  te(el, 'touchstart', 200, from, t0);
  te(el, 'touchmove', 200, from + (to - from) / 2, t0 + 200);
  te(el, 'touchmove', 200, to, t0 + 400);
  te(el, 'touchend', 200, to, t0 + 600);
};

/** the page's HTML once a scene has drawn, its machines bound and its act done (ids and counters masked) */
async function htmlOf(s: OverlayScene): Promise<string> {
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(s.view, host);
  await tick(20);
  s.act?.();
  await tick(40);
  const html = document.body.innerHTML;
  dispose();
  document.body.innerHTML = '';
  await tick(10);
  // the icon sprite the first Icon injects is the kit's, not the overlay's
  return html
    .replace(/<svg[^>]*position:absolute[^>]*>[^]*?<\/svg>/, '')
    .replace(/cl-\d+/g, 'cl-N')
    .replace(/ id="[^"]*"| aria-(controls|labelledby|describedby)="[^"]*"| for="[^"]*"| data-ownedby="[^"]*"| (data-)?(id|toast-group)="[^"]*"/g, '');
}

describe('the browser look is unchanged', () => {
  it('a_browser_render_of_every_overlay_is_byte_identical_to_before', async () => {
    // the phone-only props passed too (detents, Select): the browser ignores every one
    const extra = (kind: string): Record<string, unknown> => (kind === 'sheet' || kind === 'dialog' ? { detents: ['large'] } : kind === 'context' ? { onSelectMode: noop } : {});
    for (const s of overlayScenes(extra)) expect(await htmlOf(s), s.name).toBe(GOLDEN[s.name]);
  });

  it('the_browser_never_fetches_the_phone_halves_not_even_on_idle', async () => {
    for (const s of overlayScenes()) await htmlOf(s);
    expect(phoneOverlays.ready()).toBeUndefined();
  });

  it('the_browser_stylesheet_carries_none_of_the_overlay_variants', () => {
    const sheets = import.meta.glob<string>('../src/css/*.css', { query: '?raw', import: 'default', eager: true });
    for (const [path, raw] of Object.entries(sheets)) {
      const css = raw.replace(/\/\*[^]*?\*\//g, '');
      expect(css, path).not.toMatch(/data-platform|'sheet-menu'|'sheet-item'|'context-menu'|'context-preview'|'context-item'|'plain-tip'|data-alert|data-detent|--k-sheet-dy/);
    }
  });
});

describe('the pure geometry (phone-math.ts)', () => {
  const vp = { width: 393, height: 852, safeTop: 0, safeBottom: 0 };
  const menu = { width: 250, height: 176 };
  it('the_context_menu_opens_under_the_row_then_over_it_then_slides_the_preview_up', () => {
    const below = contextPlace({ top: 100, left: 0, width: 393, height: 72 }, vp, menu, 8, 30);
    expect(below).toEqual({ previewTop: 100, menuTop: 180, menuLeft: CONTEXT_MARGIN, below: true });
    const above = contextPlace({ top: 700, left: 0, width: 393, height: 72 }, vp, menu, 8, 30);
    expect(above.below).toBe(false);
    expect(above.menuTop).toBe(700 - 8 - 176);
    // a tall menu and a row in the middle: the row slides up until the menu fits under it
    const slid = contextPlace({ top: 300, left: 0, width: 393, height: 72 }, vp, { width: 250, height: 700 }, 8, 30);
    expect(slid.below).toBe(true);
    expect(slid.previewTop).toBe(852 - CONTEXT_MARGIN - 700 - 8 - 72);
    expect(slid.menuTop).toBe(slid.previewTop + 72 + 8);
    // a press on the far half: under the row's trailing edge
    expect(contextPlace({ top: 100, left: 0, width: 393, height: 72 }, vp, menu, 8, 300).menuLeft).toBe(393 - CONTEXT_MARGIN - 250);
  });

  it('a_plain_tooltip_sits_over_its_anchor_or_under_it_inside_the_screen', () => {
    expect(tipPlace({ top: 400, left: 100, width: 48, height: 48 }, { width: 80, height: 24 }, vp, 4)).toEqual({ top: 372, left: 84 });
    expect(tipPlace({ top: 10, left: 0, width: 48, height: 48 }, { width: 80, height: 24 }, vp, 4)).toEqual({ top: 62, left: CONTEXT_MARGIN });
    expect(TIP_MS).toBe(1500);
  });

  it('an_ios_sheet_steps_large_to_medium_to_closed_and_grows_on_a_drag_up', () => {
    expect(detentsOf(undefined)).toEqual(['medium', 'large']);
    expect(detentsOf(['large', 'medium', 'large'])).toEqual(['medium', 'large']);
    expect(detentsOf(['large'])).toEqual(['large']);
    expect(detentAfter(['medium', 'large'], 'large', 'down')).toBe('medium');
    expect(detentAfter(['medium', 'large'], 'medium', 'down')).toBe('close');
    expect(detentAfter(['large'], 'large', 'down')).toBe('close');
    expect(detentAfter(['medium', 'large'], 'medium', 'up')).toBe('large');
    expect(detentAfter(['medium'], 'medium', 'up')).toBe('medium');
  });

  it('a_long_press_menu_puts_select_first_flattens_groups_and_drops_stray_separators', () => {
    const rows = phoneRows([{ kind: 'separator' }, { value: 'a', label: 'A' }, { kind: 'separator' }, { kind: 'separator' }, { kind: 'group', id: 'g', label: 'Move', items: [{ value: 'b', label: 'B' }] }, { kind: 'group', id: 'e', label: 'Empty', items: [] }, { kind: 'separator' }], 'Select');
    expect(rows.map((r) => (r.kind === 'item' ? r.item.value : r.kind === 'label' ? `#${r.label}` : r.kind))).toEqual([SELECT_VALUE, 'separator', 'a', 'separator', '#Move', 'b']);
    expect(rowsHeight(rows, 44, 8)).toBe(44 * 4 + 8 * 2);
    expect(phoneRows([{ value: 'a', label: 'A' }])[0]).toEqual({ kind: 'item', item: { value: 'a', label: 'A' } });
  });
});

describe('ContextMenu on a long-press', () => {
  const view = (onSelect: (v: string) => void, onSelectMode?: () => void) => (): JSX.Element => (
    <ContextMenu items={[...MENU_ITEMS, { kind: 'submenu', value: 'move', label: 'Move to', items: [{ value: 'p1', label: 'wisp' }] }]} onSelect={onSelect} {...(onSelectMode ? { onSelectMode } : {})} part="list.menu">
      <div data-kit="list-row" id="row-1">
        <span data-kit="list-row-title">Fix the flaky test</span>
      </div>
    </ContextMenu>
  );

  it('android_opens_the_rows_actions_in_a_bottom_sheet_with_select_first', async () => {
    const onSelect = vi.fn();
    const onSelectMode = vi.fn();
    on('android', view(onSelect, onSelectMode));
    contextAt(q('#row-1 span'));
    await landed();
    const sheet = q("[data-kit='sheet']");
    expect(sheet?.textContent).toContain('Fix the flaky test');
    // its × is the drag-down's twin, whatever the sheet's part
    expect(q("[data-kit='sheet'] [data-part~='ui.sheet.close']")).not.toBeNull();
    expect(sheet?.getAttribute('data-gesture')).toBe('sheet.drag-down');
    const items = qa("[data-kit='sheet-item']");
    expect(items.map((i) => i.textContent)).toEqual(['Select', 'Open', 'Mark done', 'Delete task', 'Move to']);
    expect(q("[data-kit='sheet-item'][data-value='delete']")?.hasAttribute('data-danger')).toBe(true);
    // no Zag menu on a phone
    expect(q("[data-kit='menu']")).toBeNull();
    items[0]?.click();
    expect(onSelectMode).toHaveBeenCalledTimes(1);
    expect(onSelect).not.toHaveBeenCalled();
    await tick(20);
    expect(q("[data-kit='sheet-menu']")).toBeNull();
    contextAt(q('#row-1 span'));
    await tick(20);
    q("[data-kit='sheet-item'][data-value='done']")?.click();
    expect(onSelect).toHaveBeenCalledWith('done');
  });

  it('android_without_a_selection_mode_has_no_select_row', async () => {
    on('android', view(noop));
    contextAt(q('#row-1 span'));
    await landed();
    expect(qa("[data-kit='sheet-item']")[0]?.textContent).toBe('Open');
  });

  it('ios_opens_the_context_menu_with_the_row_as_its_preview', async () => {
    const onSelect = vi.fn();
    on('ios', view(onSelect, noop));
    contextAt(q('#row-1 span'));
    await landed();
    const preview = q("[data-kit='context-preview']");
    expect(preview?.getAttribute('aria-hidden')).toBe('true');
    // a copy of the pressed row, its ids taken off
    expect(preview?.textContent).toBe('Fix the flaky test');
    expect(preview?.querySelector('[id]')).toBeNull();
    expect(qa('#row-1')).toHaveLength(1);
    const menu = q("[data-kit='context-menu']");
    expect(menu?.getAttribute('role')).toBe('menu');
    expect(menu?.getAttribute('aria-label')).toBe('Fix the flaky test');
    // no Select on iOS (it's in the bar); focus on the first action
    expect(qa("[data-kit='context-item']").map((i) => i.textContent)).toEqual(['Open', 'Mark done', 'Delete task', 'Move to']);
    expect(document.activeElement?.getAttribute('data-value')).toBe('open');
    menu?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement?.getAttribute('data-value')).toBe('done');
    menu?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    expect(q("[data-kit='context-menu']")).toBeNull();
    // a tap outside closes it; a choice runs and closes
    contextAt(q('#row-1 span'));
    await tick(10);
    q("[data-kit='context-backdrop']")?.click();
    await tick();
    expect(q("[data-kit='context-menu']")).toBeNull();
    contextAt(q('#row-1 span'));
    await tick(10);
    q("[data-kit='context-item'][data-value='delete']")?.click();
    expect(onSelect).toHaveBeenCalledWith('delete');
    expect(q("[data-kit='context-menu']")).toBeNull();
  });

  for (const look of ['android', 'ios'] as const) {
    it(`${look}_a_submenu_drills_in_under_a_back_row`, async () => {
      const onSelect = vi.fn();
      on(look, view(onSelect));
      contextAt(q('#row-1 span'));
      await landed();
      const kit = look === 'ios' ? 'context-item' : 'sheet-item';
      q(`[data-kit='${kit}'][data-value='move']`)?.click();
      await tick();
      expect(qa(`[data-kit='${kit}']`).map((i) => i.textContent)).toEqual(['Move to', 'wisp']);
      q(`[data-kit='${kit}'][data-back]`)?.click();
      await tick();
      expect(qa(`[data-kit='${kit}']`)).toHaveLength(look === 'ios' ? 4 : 4);
      q(`[data-kit='${kit}'][data-value='move']`)?.click();
      await tick();
      q(`[data-kit='${kit}'][data-value='p1']`)?.click();
      expect(onSelect).toHaveBeenCalledWith('p1');
    });
  }

  it('a_rows_long_press_opens_the_lists_menu_on_that_row_the_task_rows_seam', async () => {
    // TaskRow: Swipe's onLongPress dispatches the list menu's contextmenu on the row it's handed (env.menuAt)
    vi.stubGlobal('innerWidth', 412);
    const h = fakeHost();
    const longPressed = vi.fn();
    on(
      'ios',
      () => (
        <ContextMenu items={MENU_ITEMS} onSelect={noop}>
          <Swipe
            onSwipe={noop}
            onLongPress={(row) => {
              longPressed(row);
              const r = row.getBoundingClientRect();
              row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left, clientY: r.bottom }));
            }}
          >
            <ListRow title="Port the swipe band" />
          </Swipe>
        </ContextMenu>
      ),
      h,
    );
    await landed();
    vi.useFakeTimers();
    const row = q("[data-kit='swipe']");
    if (!row) throw new Error('no row');
    pe(row, 'pointerdown', 200);
    vi.advanceTimersByTime(500);
    vi.useRealTimers();
    expect(longPressed).toHaveBeenCalledWith(row);
    expect(h.ticks).toEqual(['tap']);
    await tick(10);
    // the preview is the whole row (the swipe), lifted
    expect(q("[data-kit='context-preview'] [data-kit='swipe']")).not.toBeNull();
    expect(q("[data-kit='context-menu']")?.getAttribute('aria-label')).toBe('Port the swipe band');
  });
});

describe('Dialog and Sheet', () => {
  const foot = (): JSX.Element => (
    <>
      <Button>Cancel</Button>
      <Button variant="danger">Delete</Button>
    </>
  );
  it('a_confirm_is_marked_an_alert_on_both_phones_and_ios_draws_other_dialogs_as_sheets', async () => {
    for (const look of ['android', 'ios'] as const) {
      on(look, () => (
        <>
          <Dialog open role="alertdialog" title="Stop the run?" footer={foot()} part="t.alert" />
          <Dialog open size="sm" title="Rename" part="t.small" />
          <Dialog open title="New job" part="t.form">
            <Text>form</Text>
          </Dialog>
        </>
      ));
      await landed();
      const alert = q("[data-part='t.alert']");
      expect(alert?.getAttribute('data-kit'), look).toBe('dialog');
      expect(alert?.hasAttribute('data-alert'), look).toBe(true);
      expect(q("[data-part='t.small']")?.hasAttribute('data-alert'), look).toBe(true);
      const form = q("[data-part='t.form']");
      // Android: the M3 basic dialog; iOS: a modal task is a sheet (HIG), its part still the dialog's
      expect(form?.getAttribute('data-kit'), look).toBe(look === 'ios' ? 'sheet' : 'dialog');
      expect(form?.hasAttribute('data-alert'), look).toBe(false);
      if (look === 'ios') {
        // a form opens large; it gets the grabber and the sheet's place
        expect(form?.getAttribute('data-detent')).toBe('large');
        expect(form?.querySelector("[data-kit='sheet-handle']")).not.toBeNull();
        expect(form?.parentElement?.hasAttribute('data-sheet')).toBe(true);
        expect(q("[data-part~='t.form.close']")?.getAttribute('data-part')).toBe('t.form.close ui.sheet.close');
      }
      document.body.innerHTML = '';
    }
  });

  it('a_phone_sheet_drags_down_to_close_its_close_is_the_twin_and_a_screen_reader_gets_close', async () => {
    const h = fakeHost();
    const [open, setOpen] = createSignal(true);
    const onOpenChange = vi.fn((o: boolean) => setOpen(o));
    on(
      'android',
      () => (
        <Sheet open={open()} onOpenChange={onOpenChange} title="New task">
          <Text>draft</Text>
        </Sheet>
      ),
      h,
    );
    await landed();
    const sheet = q("[data-kit='sheet']");
    if (!sheet) throw new Error('no sheet');
    vi.spyOn(sheet, 'getBoundingClientRect').mockReturnValue({ width: 412, height: 400, top: 400, left: 0, right: 412, bottom: 800, x: 0, y: 400, toJSON: () => ({}) } as DOMRect);
    expect(sheet.getAttribute('data-gesture')).toBe('sheet.drag-down');
    expect(sheet.getAttribute('data-twin')).toBe('ui.sheet.close');
    expect(q("[data-part='ui.sheet.close']")).not.toBeNull();
    expect(h.given.get(sheet)?.map((a) => a.label)).toEqual(['Close']);
    // a short drag springs back; the sheet follows the finger while it drags
    te(sheet, 'touchstart', 200, 420, 0);
    te(sheet, 'touchmove', 200, 460, 100);
    expect(sheet.style.getPropertyValue('--k-sheet-dy')).toBe('40px');
    expect(sheet.hasAttribute('data-dragging')).toBe(true);
    te(sheet, 'touchend', 200, 460, 900);
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(sheet.style.getPropertyValue('--k-sheet-dy')).toBe('0px');
    dragDown(sheet, 420, 600, 2000);
    await tick(20);
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(h.ticks).toContain('select');
    expect(q("[data-kit='sheet']")).toBeNull();
  });

  it('an_ios_sheet_settles_on_medium_grows_on_a_drag_up_or_a_grabber_tap_and_closes_from_medium', async () => {
    const onOpenChange = vi.fn();
    on('ios', () => (
      <Sheet open onOpenChange={onOpenChange} title="Filter">
        <Text>body</Text>
      </Sheet>
    ));
    await landed();
    const sheet = q("[data-kit='sheet']");
    if (!sheet) throw new Error('no sheet');
    vi.spyOn(sheet, 'getBoundingClientRect').mockReturnValue({ width: 393, height: 426, top: 426, left: 0, right: 393, bottom: 852, x: 0, y: 426, toJSON: () => ({}) } as DOMRect);
    expect(sheet.getAttribute('data-detent')).toBe('medium');
    // a drag up grows it to large
    te(sheet, 'touchstart', 200, 500, 0);
    te(sheet, 'touchmove', 200, 450, 100);
    te(sheet, 'touchmove', 200, 300, 200);
    te(sheet, 'touchend', 200, 300, 400);
    expect(sheet.getAttribute('data-detent')).toBe('large');
    // a drag down from large settles on medium, not closed
    dragDown(sheet, 300, 500, 1000);
    expect(sheet.getAttribute('data-detent')).toBe('medium');
    expect(onOpenChange).not.toHaveBeenCalled();
    // a finger's tap on the grabber steps them too (UIKit)
    const tapGrabber = (): void => {
      const h = q("[data-kit='sheet-handle']") as HTMLElement;
      pe(h, 'pointerdown', 196, 432);
      pe(h, 'pointerup', 196, 432);
      h.click();
    };
    tapGrabber();
    expect(sheet.getAttribute('data-detent')).toBe('large');
    tapGrabber();
    expect(onOpenChange).not.toHaveBeenCalled();
    dragDown(sheet, 450, 650, 3000);
    await tick(10);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

describe('review fixes', () => {
  it('an_ios_sheet_at_large_lets_a_drag_up_scroll_its_content', async () => {
    on('ios', () => (
      <Sheet open onOpenChange={noop} title="Long" detents={['large']}>
        <Text>body</Text>
      </Sheet>
    ));
    await landed();
    const sheet = q("[data-kit='sheet']");
    if (!sheet) throw new Error('no sheet');
    expect(sheet.getAttribute('data-detent')).toBe('large');
    const start = new Event('touchstart', { bubbles: true, cancelable: true });
    Object.defineProperties(start, { touches: { value: [{ clientX: 200, clientY: 500 }] }, changedTouches: { value: [{ clientX: 200, clientY: 500 }] } });
    sheet.dispatchEvent(start);
    const move = new Event('touchmove', { bubbles: true, cancelable: true });
    Object.defineProperties(move, { touches: { value: [{ clientX: 200, clientY: 420 }] }, changedTouches: { value: [{ clientX: 200, clientY: 420 }] } });
    sheet.dispatchEvent(move);
    // the browser scrolls the content: nothing prevented, the sheet doesn't move
    expect(move.defaultPrevented).toBe(false);
    expect(sheet.style.getPropertyValue('--k-sheet-dy')).toBe('');
  });

  it('the_ios_context_menu_preview_is_inert_and_focus_returns_and_escape_works_from_anywhere', async () => {
    on('ios', () => (
      <>
        <button type="button" id="before">before</button>
        <ContextMenu items={MENU_ITEMS} onSelect={noop}>
          <div data-kit="list-row" id="row-1" data-gesture="row.swipe" data-part="card.head">
            <button type="button" id="inner">Done</button>
          </div>
        </ContextMenu>
      </>
    ));
    q('#before')?.focus();
    contextAt(q('#row-1'));
    await landed();
    const clone = q("[data-kit='context-preview'] [data-kit='list-row']") as (HTMLElement & { inert?: boolean }) | null;
    expect(clone?.inert).toBe(true);
    expect(q("[data-kit='context-preview'] [data-gesture], [data-kit='context-preview'] [data-part], [data-kit='context-preview'] [id]")).toBeNull();
    expect(q("[data-kit='context-menu']")?.getAttribute('aria-modal')).toBe('true');
    // Escape reaches it with focus anywhere, and focus goes back where it was
    document.body.focus();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick();
    expect(q("[data-kit='context-menu']")).toBeNull();
    expect(document.activeElement?.id).toBe('before');
  });

  it('a_browser_right_click_before_the_machine_still_replays_on_the_area', async () => {
    // unchanged for the browser: the early press is replayed on the area, so a row re-rendered by the press can't lose it
    const onOpenChange = vi.fn();
    mount(() => (
      <ContextMenu items={MENU_ITEMS} onSelect={noop} onOpenChange={onOpenChange}>
        <span id="r">row</span>
      </ContextMenu>
    ));
    const r = q('#r');
    contextAt(r);
    r?.remove();
    await preloadControls();
    await tick(40);
    expect(q("[data-kit='menu']")?.getAttribute('data-state')).toBe('open');
  });

  it('a_popover_without_a_title_is_named_by_its_trigger_on_a_phone', async () => {
    on('android', () => (
      <Popover open trigger={{ icon: 'search', 'aria-label': 'Filter' }} part="t.untitled">
        <Text>body</Text>
      </Popover>
    ));
    await landed();
    expect(q("[data-part='t.untitled.title']")?.textContent).toBe('Filter');
  });
});

describe('Popover', () => {
  it('a_popover_is_a_bottom_sheet_on_a_phone_and_stays_a_popover_on_an_ipad', async () => {
    on('android', () => (
      <Popover trigger={{ label: 'Filter' }} title="Filter" part="t.pop">
        <Text>body</Text>
      </Popover>
    ));
    await landed();
    const trigger = q("[data-part='t.pop.trigger']");
    expect(trigger?.getAttribute('aria-haspopup')).toBe('dialog');
    trigger?.click();
    // before the next frame (Zag answers the press in a microtask): its surface is there
    for (let i = 0; i < 5; i++) await Promise.resolve();
    expect(q("[data-part='t.pop']")?.getAttribute('data-kit')).toBe('sheet');
    expect(q("[data-part='t.pop']")?.textContent).toContain('body');
    expect(q("[data-kit='popover']")).toBeNull();
    document.body.innerHTML = '';
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('600px'), media: query, addEventListener: noop, removeEventListener: noop }) as unknown as MediaQueryList);
    on('ios', () => (
      <Popover open trigger={{ label: 'Filter' }} title="Filter" part="t.ipad">
        <Text>body</Text>
      </Popover>
    ));
    await tick(20);
    expect(q("[data-part='t.ipad']")?.getAttribute('data-kit')).toBe('popover');
  });
});

describe('Tooltip', () => {
  it('android_a_long_press_shows_the_plain_tooltip_for_a_moment_and_opens_nothing', async () => {
    const h = fakeHost();
    const pressed = vi.fn();
    on(
      'android',
      () => (
        <Tooltip content="Mark done" hint="D">
          <button type="button" aria-label="Mark done" onClick={pressed} id="done" />
        </Tooltip>
      ),
      h,
    );
    await landed();
    const trigger = q("[data-kit='tooltip-trigger']");
    const button = q('#done');
    if (!trigger || !button) throw new Error('no trigger');
    // no Zag machine on a phone
    expect(trigger.hasAttribute('data-scope')).toBe(false);
    vi.useFakeTimers();
    pe(button, 'pointerdown', 50);
    vi.advanceTimersByTime(500);
    const tip = q("[data-kit='plain-tip']");
    expect(tip?.getAttribute('role')).toBe('tooltip');
    expect(tip?.textContent).toBe('Mark done D');
    expect(h.ticks).toEqual(['tap']);
    pe(button, 'pointerup', 50);
    button.click();
    expect(pressed).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1499);
    expect(q("[data-kit='plain-tip']")).not.toBeNull();
    vi.advanceTimersByTime(1);
    expect(q("[data-kit='plain-tip']")).toBeNull();
    // a tap is a tap
    pe(button, 'pointerdown', 50);
    pe(button, 'pointerup', 50);
    button.click();
    expect(pressed).toHaveBeenCalledTimes(1);
    expect(q("[data-kit='plain-tip']")).toBeNull();
  });

  it('ios_has_no_tooltip_at_all', async () => {
    on('ios', () => (
      <Tooltip content="Mark done">
        <button type="button" id="done" />
      </Tooltip>
    ));
    await landed();
    vi.useFakeTimers();
    const button = q('#done');
    if (!button) throw new Error('no button');
    pe(button, 'pointerdown', 50);
    vi.advanceTimersByTime(800);
    expect(q("[data-kit='plain-tip'], [data-kit='tooltip']")).toBeNull();
    expect(q("[data-kit='tooltip-trigger']")?.hasAttribute('data-scope')).toBe(false);
  });
});

describe('Toasts sit above the navigation bar and the FAB', () => {
  it('the_nav_bar_and_the_fab_mark_the_body_while_they_are_up', () => {
    const [shown, setShown] = createSignal(true);
    on('android', () => (
      <>
        {shown() && <NavigationBar aria-label="Destinations" items={[{ id: 'tasks', label: 'Tasks', icon: 'feed' }]} selected="tasks" onSelect={noop} />}
        {shown() && <Fab label="New task" icon="plus" onClick={noop} />}
      </>
    ));
    expect(document.body.hasAttribute('data-nav-bar')).toBe(true);
    expect(document.body.hasAttribute('data-fab')).toBe(true);
    setShown(false);
    expect(document.body.hasAttribute('data-nav-bar')).toBe(false);
    expect(document.body.hasAttribute('data-fab')).toBe(false);
  });

  it('a_mark_held_twice_stays_until_both_let_go_and_a_browser_page_gets_none', () => {
    const a = markRoot('data-x');
    const b = markRoot('data-x');
    a();
    a();
    expect(document.body.hasAttribute('data-x')).toBe(true);
    b();
    expect(document.body.hasAttribute('data-x')).toBe(false);
    mount(() => <NavigationBar aria-label="Destinations" items={[]} selected="" onSelect={noop} />);
    expect(document.body.hasAttribute('data-nav-bar')).toBe(false);
  });

  it('each_look_places_the_toast_over_its_bars_and_draws_its_own_shape', () => {
    const sheets = import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true });
    const css = (look: string): string => Object.entries(sheets).find(([p]) => p.endsWith(`${look}.css`))?.[1] ?? '';
    expect(css('android')).toMatch(/body\[data-nav-bar\]\[data-fab\] \{ --toast-bottom: calc\(var\(--nav-h\) \+ var\(--fab-ext-h\) \+ var\(--fab-margin\) \+ var\(--snack-margin\)\); \}/);
    expect(css('android')).toMatch(/\[data-kit='toast'\] \{[^}]*background: var\(--inverse-surface\)/);
    expect(css('ios')).toMatch(/body\[data-nav-bar\] \{ --toast-bottom: calc\(var\(--nav-h\) \+ var\(--capsule-gap\)\); \}/);
    expect(css('ios')).toMatch(/\[data-kit='toast'\] \{[^}]*border-radius: var\(--radius-pill\)/);
  });
});

// QA r7 #1: a screen reader can close a phone sheet. Its grabber is a button named Close (focusable, VoiceOver's and
// TalkBack's activation: a click with no finger on it, as the Enter key's); a finger's tap still steps an iOS sheet's
// detents; Escape closes it; the browser keeps its plain handle (the golden)
describe('the grabber closes the sheet for a screen reader (QA r7 #1)', () => {
  for (const look of ['ios', 'android'] as const) {
    it(`on_${look}_the_grabber_is_a_focusable_button_named_close_and_its_activation_closes_the_sheet`, async () => {
      const onOpenChange = vi.fn();
      on(look, () => (
        <Sheet open onOpenChange={onOpenChange} title="Changes" detents={['medium', 'large']}>
          <Text>body</Text>
        </Sheet>
      ));
      await landed();
      const h = q("[data-kit='sheet'] > [data-kit='sheet-handle']") as HTMLElement;
      expect(h.tagName).toBe('BUTTON');
      expect(h.getAttribute('aria-label')).toBe('Close');
      expect(h.getAttribute('aria-hidden')).toBeNull();
      expect(h.tabIndex).toBe(0);
      // VoiceOver / TalkBack / Enter: a click no finger made
      h.click();
      await tick(10);
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  }

  it('on_android_a_finger_tap_on_the_handle_closes_the_sheet_and_the_end_of_a_drag_does_not', async () => {
    const onOpenChange = vi.fn();
    on('android', () => (
      <Sheet open onOpenChange={onOpenChange} title="Changes">
        <Text>body</Text>
      </Sheet>
    ));
    await landed();
    const h = q("[data-kit='sheet-handle']") as HTMLElement;
    // a drag that starts on the handle and lets go elsewhere: its click is not a tap
    pe(h, 'pointerdown', 180, 300);
    pe(h, 'pointerup', 180, 360);
    h.click();
    await tick(10);
    expect(onOpenChange).not.toHaveBeenCalled();
    pe(h, 'pointerdown', 180, 300);
    pe(h, 'pointerup', 180, 302);
    h.click();
    await tick(10);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('on_ios_escape_closes_a_sheet', async () => {
    const onOpenChange = vi.fn();
    on('ios', () => (
      <Sheet open onOpenChange={onOpenChange} title="Changes" detents={['medium', 'large']}>
        <Text>body</Text>
      </Sheet>
    ));
    await landed();
    q("[data-kit='sheet']")?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await tick(10);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('an_ios_dialog_drawn_as_a_sheet_gets_the_same_close_grabber', async () => {
    const onOpenChange = vi.fn();
    on('ios', () => (
      <Dialog open onOpenChange={onOpenChange} title="Rename">
        <Text>body</Text>
      </Dialog>
    ));
    await landed();
    const h = q("[data-kit='sheet'] > [data-kit='sheet-handle']") as HTMLElement;
    expect(h.tagName).toBe('BUTTON');
    expect(h.getAttribute('aria-label')).toBe('Close');
    h.click();
    await tick(10);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

// QA r7 #7: in a phone menu whose rows have icons, a row without one keeps the icon column (an empty slot), and a
// choice's ✓ sits in that leading column (M3's selected item, UIKit's checked action), not at the screen's edge
describe('a phone menu keeps its icon column (QA r7 #7)', () => {
  const items = [
    { value: 'details', label: 'Details', icon: 'info' as const },
    { value: 'lean:follow', label: 'Lean runs · As in Settings', checked: true },
    { value: 'lean:on', label: 'Lean runs · On for this task', checked: false },
    { value: 'plain', label: 'No icon' },
  ];
  for (const look of ['android', 'ios'] as const) {
    it(`on_${look}_rows_without_an_icon_keep_the_column_and_a_choice_is_checked_at_its_start`, async () => {
      on(look, () => (
        <ContextMenu items={items} onSelect={noop} part="list.menu">
          <div data-kit="list-row" id="row-1">
            <span data-kit="list-row-title">Fix the flaky test</span>
          </div>
        </ContextMenu>
      ));
      contextAt(q('#row-1 span'));
      await landed();
      const kit = look === 'android' ? 'sheet' : 'context';
      const row = (v: string): HTMLElement => q(`[data-kit='${kit}-item'][data-value='${v}']`) as HTMLElement;
      const lead = (v: string): Element | null => row(v).firstElementChild;
      // Android's icons lead; iOS's trail (UIKit), its leading column only the choices'
      if (look === 'android') expect(lead('details')?.getAttribute('data-kit')).toBe('icon');
      else expect([lead('details')?.getAttribute('data-kit'), row('details').querySelector('[data-icon="info"]') !== null]).toEqual(['context-icon-gap', true]);
      // the checked choice: its ✓ first, none at the end
      expect(lead('lean:follow')?.getAttribute('data-icon')).toBe('check');
      expect(row('lean:follow').lastElementChild?.getAttribute('data-icon')).not.toBe('check');
      expect(row('lean:follow').querySelectorAll('[data-icon="check"]').length).toBe(1);
      // the unchecked one and the plain one: an empty slot where the icon goes
      expect(lead('lean:on')?.getAttribute('data-kit')).toBe(`${kit}-icon-gap`);
      expect(lead('plain')?.getAttribute('data-kit')).toBe(`${kit}-icon-gap`);
    });
  }

  it('a_menu_with_no_icons_at_all_draws_no_empty_column', async () => {
    on('android', () => (
      <ContextMenu items={[{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]} onSelect={noop} part="list.menu">
        <div data-kit="list-row" id="row-1">
          <span data-kit="list-row-title">Row</span>
        </div>
      </ContextMenu>
    ));
    contextAt(q('#row-1 span'));
    await landed();
    expect(qa("[data-kit='sheet-icon-gap']")).toEqual([]);
  });
});
