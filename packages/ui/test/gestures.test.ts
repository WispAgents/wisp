// ADR-0065 build step 7: the kit's gesture registry and its binders (design/mobile.md §A.4–§A.6). The ADR's three
// gesture edge cases are named here (every_gesture_names_its_control, an_edge_swipe_is_the_systems,
// a_swipe_in_a_scroller_scrolls_it); the rest pin each gesture's rules: what commits, what ticks, what it ignores.
// The components that carry them (Swipe, VirtualList, TopAppBar) are in gestures-wiring.test.tsx.
// size-ok: one file per ADR test name (the ADR cites gestures.test.ts), each gesture's rules beside the others.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { GESTURES, defineGestures, gestureMarks, type GestureSpec } from '../src/gestures/registry.ts';
import { GESTURE_EDGE, LONG_PRESS_MS, PAGE_SWIPE_LINE, PULL_LINE, PULL_RESIST, edgeIsBack, edgeOf, pageSwipeRelease, pullProgress, reorderSlot, sheetCloses, stepBefore, touchOwner } from '../src/gestures/gesture-math.ts';
import { bindLongPress, bindPageSwipe } from '../src/gestures/bind.ts';
import { bindPull, bindReorderDrag, bindSheetDrag } from '../src/gestures/bind-vertical.ts';
import type { GestureHost, Haptic } from '../src/gestures/host.tsx';
import { reordered } from '../src/core/reorder.ts';

const W = 412;

afterEach(() => {
  document.body.innerHTML = '';
});

function host(): GestureHost & { readonly ticks: Haptic[] } {
  const ticks: Haptic[] = [];
  return { ticks, haptic: (k) => void ticks.push(k) };
}

function box(html = ''): HTMLElement {
  const el = document.createElement('div');
  el.innerHTML = html;
  document.body.append(el);
  return el;
}

/** A touch pointer event; returns it so a test can ask whether it was prevented. */
function pe(el: Element, type: string, x: number, y = 300, t = 0): PointerEvent {
  const e = new PointerEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, pointerType: 'touch', isPrimary: true, pointerId: 1 } as PointerEventInit);
  Object.defineProperty(e, 'timeStamp', { value: t });
  el.dispatchEvent(e);
  return e;
}

function swipe(el: Element, from: number, to: number, target: Element = el): void {
  pe(target, 'pointerdown', from);
  for (let i = 1; i <= 8; i++) pe(target, 'pointermove', from + ((to - from) * i) / 8, 300, i * 40);
  pe(target, 'pointerup', to, 300, 400);
}

/** A touch event at (x, y): happy-dom's TouchEvent without Touch objects, so the list is set by hand. */
function te(el: Element, type: string, x: number, y: number, t = 0): Event {
  const e = new Event(type, { bubbles: true, cancelable: true });
  const touches = type === 'touchend' || type === 'touchcancel' ? [] : [{ clientX: x, clientY: y }];
  Object.defineProperties(e, { touches: { value: touches }, changedTouches: { value: [{ clientX: x, clientY: y }] }, timeStamp: { value: t } });
  el.dispatchEvent(e);
  return e;
}

/** A fake timer for the long-presses: `run()` fires what's due. */
function timer(): { schedule: (fn: () => void, ms: number) => () => void; run: () => void; pending: () => number } {
  let jobs: { fn: () => void; ms: number }[] = [];
  return {
    schedule: (fn, ms) => {
      const job = { fn, ms };
      jobs.push(job);
      return () => void (jobs = jobs.filter((j) => j !== job));
    },
    run: () => {
      const due = jobs;
      jobs = [];
      for (const j of due) j.fn();
    },
    pending: () => jobs.length,
  };
}

describe('the registry', () => {
  it('every_gesture_names_its_control', () => {
    // the type: a gesture without a twin, or with no twin part, doesn't compile (npm run typecheck checks these)
    // @ts-expect-error a gesture must name its twin
    defineGestures({ x: { kind: 'long-press', does: 'nothing visible' } });
    // @ts-expect-error a twin names at least one control on screen
    defineGestures({ x: { kind: 'long-press', does: 'x', twin: { command: 'ui.x', parts: [] } } });
    // @ts-expect-error a twin names the command its control runs
    defineGestures({ x: { kind: 'long-press', does: 'x', twin: { parts: ['ui.x'] } } });
    // and every entry of the kit's registry does, with real data-part names (namespace.name)
    const names = Object.keys(GESTURES);
    expect(names.sort()).toEqual(['list.pull', 'queue.drag', 'row.long-press', 'row.swipe', 'sheet.drag-down', 'story.drag-down', 'story.long-press', 'stuff.swipe', 'task.body-swipe', 'task.head-swipe']);
    for (const [name, g] of Object.entries(GESTURES)) {
      expect(g.twin.command, name).toMatch(/^[a-z-]+(\.[a-zA-Z-]+)+$/);
      expect(g.twin.parts.length, name).toBeGreaterThan(0);
      for (const p of g.twin.parts) expect(p, name).toMatch(/^[a-z-]+(\.[a-z-]+)+$/);
    }
  });

  it('an_element_with_two_gestures_names_both_and_their_twins_once', () => {
    expect(gestureMarks(['row.swipe', 'row.long-press'])).toEqual({ 'data-gesture': 'row.swipe row.long-press', 'data-twin': 'ui.app-bar.actions' });
    expect(gestureMarks(['queue.drag'])).toEqual({ 'data-gesture': 'queue.drag', 'data-twin': 'composer.queue.grip' });
  });

  it('haptics_are_select_on_a_line_and_tap_on_a_long_press', () => {
    for (const [name, g] of Object.entries(GESTURES) as [string, GestureSpec][]) {
      if (g.haptic?.line) expect(g.haptic.line, name).toBe('select');
      if (g.haptic?.start) expect(g.haptic.start, name).toBe('tap');
    }
    expect(GESTURES['row.long-press'].haptic.start).toBe('tap');
  });
});

describe('the edges are the system’s', () => {
  it('an_edge_swipe_is_the_systems', () => {
    // the rule: 24 px on both sides is the system's; Back is Android's either edge, iOS's left
    expect(GESTURE_EDGE).toBe(24);
    expect(edgeOf(0, W)).toBe('left');
    expect(edgeOf(24, W)).toBe('left');
    expect(edgeOf(25, W)).toBeUndefined();
    expect(edgeOf(W - 25, W)).toBeUndefined();
    expect(edgeOf(W - 24, W)).toBe('right');
    expect(touchOwner(10, W, false)).toBe('system');
    expect(touchOwner(10, W, true)).toBe('system');
    expect(edgeIsBack('left', 'ios')).toBe(true);
    expect(edgeIsBack('right', 'ios')).toBe(false);
    expect(edgeIsBack('right', 'android')).toBe(true);
    // on the page: a swipe from either edge zone opens no side panel, and the page leaves the touch alone
    // (no capture, nothing prevented) so the system's Back gets it
    vi.stubGlobal('innerWidth', W);
    const page = box();
    const capture = vi.fn();
    page.setPointerCapture = capture;
    const h = host();
    const onSwipe = vi.fn();
    const stop = bindPageSwipe(page, { host: h, can: (d) => d === 'left', onSwipe });
    for (const start of [10, W - 10]) {
      const down = pe(page, 'pointerdown', start);
      const move = pe(page, 'pointermove', start - 60);
      pe(page, 'pointermove', start - 150);
      pe(page, 'pointerup', start - 150);
      expect(down.defaultPrevented || move.defaultPrevented).toBe(false);
    }
    expect(onSwipe).not.toHaveBeenCalled();
    expect(capture).not.toHaveBeenCalled();
    expect(h.ticks).toEqual([]);
    // a long-press and a pull from the edge are the system's too
    const t = timer();
    const onLong = vi.fn();
    bindLongPress(page, { host: h, onLongPress: onLong, schedule: t.schedule });
    pe(page, 'pointerdown', 8);
    expect(t.pending()).toBe(0);
    // and the same swipe from inside the zones opens it
    swipe(page, 300, 300 - PAGE_SWIPE_LINE - 20);
    expect(onSwipe).toHaveBeenCalledWith('left');
    stop();
    vi.unstubAllGlobals();
  });
});

describe('scrollers keep their swipes', () => {
  it('a_swipe_in_a_scroller_scrolls_it', () => {
    vi.stubGlobal('innerWidth', W);
    // a code block, a diff and a chip row (any overflow-x box that overflows) inside the task page
    const page = box('<figure data-kit="code-block"><code>let x = 1;</code></figure><div data-kit="diff"><span>+a</span></div><div id="chips" style="overflow-x: auto"><span>chip</span></div><p id="text">words</p>');
    const chips = page.querySelector<HTMLElement>('#chips');
    if (!chips) throw new Error('no chips');
    Object.defineProperty(chips, 'scrollWidth', { value: 900, configurable: true });
    Object.defineProperty(chips, 'clientWidth', { value: 380 });
    const onSwipe = vi.fn();
    bindPageSwipe(page, { host: host(), can: () => true, onSwipe });
    for (const sel of ['code', '[data-kit="diff"] span', '#chips span']) {
      const target = page.querySelector(sel);
      if (!target) throw new Error(sel);
      swipe(page, 300, 120, target);
    }
    expect(onSwipe).not.toHaveBeenCalled();
    expect(touchOwner(200, W, true)).toBe('scroller');
    // a chip row that fits doesn't scroll, so it swipes the page; and so does plain text
    Object.defineProperty(chips, 'scrollWidth', { value: 300 });
    swipe(page, 300, 120, page.querySelector('#chips span') ?? page);
    swipe(page, 300, 120, page.querySelector('#text') ?? page);
    expect(onSwipe).toHaveBeenCalledTimes(2);
    vi.unstubAllGlobals();
  });
});

describe('page swipes (task head, task body)', () => {
  it('a_page_swipe_commits_past_its_line_or_on_a_flick_and_ticks_once', () => {
    expect(pageSwipeRelease(-PAGE_SWIPE_LINE, 0)).toBe('commit');
    expect(pageSwipeRelease(-40, 0)).toBe('cancel');
    expect(pageSwipeRelease(-40, -0.8)).toBe('commit');
    expect(pageSwipeRelease(-40, 0.8)).toBe('cancel');
    vi.stubGlobal('innerWidth', W);
    const page = box();
    const h = host();
    const onSwipe = vi.fn();
    const onDrag = vi.fn();
    bindPageSwipe(page, { host: h, can: () => true, onSwipe, onDrag });
    swipe(page, 300, 300 - PAGE_SWIPE_LINE - 30);
    expect(onSwipe).toHaveBeenCalledWith('left');
    expect(h.ticks).toEqual(['select']);
    // the page followed, then came back
    expect(onDrag.mock.calls.at(-1)).toEqual([0]);
    vi.unstubAllGlobals();
  });

  it('a_way_with_nothing_there_and_a_vertical_drag_do_nothing', () => {
    vi.stubGlobal('innerWidth', W);
    const page = box();
    const onSwipe = vi.fn();
    bindPageSwipe(page, { host: host(), can: (d) => d === 'left', onSwipe });
    // rightward: the body has nothing that way
    swipe(page, 100, 300);
    // vertical: a scroll
    pe(page, 'pointerdown', 200, 300);
    pe(page, 'pointermove', 205, 200);
    pe(page, 'pointermove', 100, 150);
    pe(page, 'pointerup', 100, 150);
    // a mouse drag is a text selection
    page.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, clientX: 300, pointerType: 'mouse', isPrimary: true } as PointerEventInit));
    page.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, clientX: 100, pointerType: 'mouse', isPrimary: true } as PointerEventInit));
    page.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, clientX: 100, pointerType: 'mouse', isPrimary: true } as PointerEventInit));
    expect(onSwipe).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });

  it('a_swipe_on_a_control_is_the_controls', () => {
    vi.stubGlobal('innerWidth', W);
    const page = box('<button type="button">Ship it</button><textarea></textarea>');
    const onSwipe = vi.fn();
    bindPageSwipe(page, { host: host(), can: () => true, onSwipe });
    swipe(page, 300, 100, page.querySelector('button') ?? page);
    swipe(page, 300, 100, page.querySelector('textarea') ?? page);
    expect(onSwipe).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});

describe('long-press', () => {
  it('a_long_press_opens_its_menu_with_a_tap_and_swallows_the_click', () => {
    vi.stubGlobal('innerWidth', W);
    const row = box();
    const h = host();
    const t = timer();
    const onLongPress = vi.fn();
    const onClick = vi.fn();
    row.addEventListener('click', onClick);
    bindLongPress(row, { host: h, onLongPress, schedule: t.schedule });
    pe(row, 'pointerdown', 200);
    t.run();
    expect(onLongPress).toHaveBeenCalledTimes(1);
    expect(h.ticks).toEqual(['tap']);
    pe(row, 'pointerup', 200);
    row.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    expect(onClick).not.toHaveBeenCalled();
    // Android's own contextmenu after it is the same press: prevented, not run twice
    const cm = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    pe(row, 'pointerdown', 200);
    t.run();
    row.dispatchEvent(cm);
    expect(cm.defaultPrevented).toBe(true);
    expect(onLongPress).toHaveBeenCalledTimes(2);
    vi.unstubAllGlobals();
  });

  it('moving_or_lifting_before_the_delay_is_no_long_press', () => {
    expect(LONG_PRESS_MS).toBe(500);
    vi.stubGlobal('innerWidth', W);
    const row = box();
    const t = timer();
    const onLongPress = vi.fn();
    bindLongPress(row, { host: host(), onLongPress, schedule: t.schedule });
    pe(row, 'pointerdown', 200);
    pe(row, 'pointermove', 220);
    expect(t.pending()).toBe(0);
    pe(row, 'pointerdown', 200);
    pe(row, 'pointerup', 200);
    expect(t.pending()).toBe(0);
    t.run();
    expect(onLongPress).not.toHaveBeenCalled();
    // an Android contextmenu during a held press runs it at once (and only once)
    pe(row, 'pointerdown', 200);
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    t.run();
    expect(onLongPress).toHaveBeenCalledTimes(1);
    vi.unstubAllGlobals();
  });
});

describe('pull to refresh', () => {
  it('a_pull_past_the_line_reconnects_now_with_one_tick', () => {
    expect(pullProgress(-10)).toBe(0);
    expect(pullProgress(PULL_LINE / PULL_RESIST)).toBe(1);
    vi.stubGlobal('innerWidth', W);
    const list = box();
    const h = host();
    const onRefresh = vi.fn();
    const seen: number[] = [];
    bindPull(list, { host: h, onRefresh, onProgress: (p) => void seen.push(p), refreshing: () => false });
    te(list, 'touchstart', 200, 100);
    te(list, 'touchmove', 200, 120);
    const move = te(list, 'touchmove', 200, 100 + PULL_LINE / PULL_RESIST + 20);
    expect(move.defaultPrevented).toBe(true);
    te(list, 'touchmove', 200, 100 + PULL_LINE / PULL_RESIST + 40);
    te(list, 'touchend', 200, 100 + PULL_LINE / PULL_RESIST + 40);
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(h.ticks).toEqual(['select']);
    expect(Math.max(...seen)).toBeGreaterThan(1);
    expect(seen.at(-1)).toBe(0);
    vi.unstubAllGlobals();
  });

  it('a_short_pull_a_scrolled_list_or_a_refresh_on_its_way_reconnects_nothing', () => {
    vi.stubGlobal('innerWidth', W);
    const list = box();
    let busy = false;
    const onRefresh = vi.fn();
    bindPull(list, { host: host(), onRefresh, onProgress: () => undefined, refreshing: () => busy });
    // short
    te(list, 'touchstart', 200, 100);
    te(list, 'touchmove', 200, 150);
    te(list, 'touchend', 200, 150);
    // scrolled down: the drag is the list's scroll
    list.scrollTop = 40;
    Object.defineProperty(list, 'scrollTop', { value: 40, configurable: true });
    te(list, 'touchstart', 200, 100);
    const scroll = te(list, 'touchmove', 200, 400);
    te(list, 'touchend', 200, 400);
    expect(scroll.defaultPrevented).toBe(false);
    Object.defineProperty(list, 'scrollTop', { value: 0, configurable: true });
    // already refreshing
    busy = true;
    te(list, 'touchstart', 200, 100);
    te(list, 'touchmove', 200, 400);
    te(list, 'touchend', 200, 400);
    // upward
    busy = false;
    te(list, 'touchstart', 200, 400);
    te(list, 'touchmove', 200, 100);
    te(list, 'touchend', 200, 100);
    expect(onRefresh).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});

describe('a sheet dragged down', () => {
  it('a_sheet_dragged_past_its_share_closes_and_a_short_drag_springs_back', () => {
    expect(sheetCloses(100, 300, 0)).toBe(true);
    expect(sheetCloses(60, 300, 0)).toBe(false);
    expect(sheetCloses(60, 300, 0.9)).toBe(true);
    expect(sheetCloses(-200, 300, 0)).toBe(false);
    const sheet = box('<div id="content"></div>');
    sheet.getBoundingClientRect = () => ({ height: 400, width: 412, top: 400, left: 0, right: 412, bottom: 800, x: 0, y: 400, toJSON: () => ({}) });
    const h = host();
    const onClose = vi.fn();
    const drags: number[] = [];
    let content: HTMLElement | undefined = sheet.querySelector<HTMLElement>('#content') ?? undefined;
    bindSheetDrag(sheet, { host: h, onClose, onDrag: (dy) => void drags.push(dy), content: () => content });
    te(sheet, 'touchstart', 200, 420, 0);
    te(sheet, 'touchmove', 200, 450, 200);
    te(sheet, 'touchend', 200, 450, 400);
    expect(onClose).not.toHaveBeenCalled();
    te(sheet, 'touchstart', 200, 420, 1000);
    te(sheet, 'touchmove', 200, 480, 1200);
    te(sheet, 'touchmove', 200, 560, 1400);
    te(sheet, 'touchend', 200, 560, 1600);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(h.ticks).toEqual(['select']);
    expect(drags.at(-1)).toBe(0);
    // the sheet's content scrolled down: the drag scrolls it back up, the sheet stays
    content = box();
    Object.defineProperty(content, 'scrollTop', { value: 120 });
    te(sheet, 'touchstart', 200, 420, 2000);
    te(sheet, 'touchmove', 200, 700, 2200);
    te(sheet, 'touchend', 200, 700, 2400);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

describe('drag to reorder the queue', () => {
  const ROW = 48;
  function queue(ids: readonly string[]): HTMLElement {
    const list = box(ids.map((id) => `<div data-part="composer.queue.row" id="r-${id}"><span>${id}</span></div>`).join(''));
    ids.forEach((id, i) => {
      const el = list.querySelector<HTMLElement>(`#r-${id}`);
      if (el) el.getBoundingClientRect = () => ({ top: i * ROW, height: ROW, bottom: (i + 1) * ROW, left: 0, right: W, width: W, x: 0, y: i * ROW, toJSON: () => ({}) });
    });
    return list;
  }

  it('a_long_pressed_item_dragged_down_two_slots_lands_there', () => {
    expect(reorderSlot([24, 72, 120], 10)).toBe(0);
    expect(reorderSlot([24, 72, 120], 100)).toBe(2);
    expect(reorderSlot([24, 72, 120], 500)).toBe(3);
    vi.stubGlobal('innerWidth', W);
    const ids = ['a', 'b', 'c', 'd'];
    const list = queue(ids);
    const h = host();
    const t = timer();
    const onReorder = vi.fn();
    bindReorderDrag(list, { host: h, item: "[data-part~='composer.queue.row']", ids: () => ids, onReorder, schedule: t.schedule });
    const a = list.querySelector('#r-a span') ?? list;
    te(a, 'touchstart', 200, 24);
    t.run();
    expect(list.querySelector('#r-a')?.hasAttribute('data-dragging')).toBe(true);
    const move = te(a, 'touchmove', 200, 130);
    expect(move.defaultPrevented).toBe(true);
    expect(list.querySelector<HTMLElement>('#r-a')?.style.getPropertyValue('--k-drag-y')).toBe('106px');
    te(a, 'touchend', 200, 130);
    // past b's and c's centres: before d
    expect(onReorder).toHaveBeenCalledWith('a', 'd');
    expect(reordered(ids, 'a', 'd')).toEqual(['b', 'c', 'a', 'd']);
    expect(h.ticks[0]).toBe('tap');
    expect(h.ticks.slice(1).every((k) => k === 'select')).toBe(true);
    expect(list.querySelector('#r-a')?.hasAttribute('data-dragging')).toBe(false);
    vi.unstubAllGlobals();
  });

  it('a_drag_before_the_press_is_long_scrolls_and_moves_nothing', () => {
    vi.stubGlobal('innerWidth', W);
    const ids = ['a', 'b'];
    const list = queue(ids);
    const t = timer();
    const onReorder = vi.fn();
    bindReorderDrag(list, { host: host(), item: "[data-part~='composer.queue.row']", ids: () => ids, onReorder, schedule: t.schedule });
    const a = list.querySelector('#r-a') ?? list;
    te(a, 'touchstart', 200, 24);
    const move = te(a, 'touchmove', 200, 90);
    t.run();
    te(a, 'touchend', 200, 90);
    expect(move.defaultPrevented).toBe(false);
    expect(onReorder).not.toHaveBeenCalled();
    // let go where it started: nothing moved, nothing sent
    te(a, 'touchstart', 200, 24);
    t.run();
    te(a, 'touchend', 200, 24);
    expect(onReorder).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });

  it('the_webviews_own_long_press_never_cancels_a_queue_drag', () => {
    vi.stubGlobal('innerWidth', W);
    const ids = ['a', 'b', 'c'];
    const list = queue(ids);
    const t = timer();
    const onReorder = vi.fn();
    bindReorderDrag(list, { host: host(), item: "[data-part~='composer.queue.row']", ids: () => ids, onReorder, schedule: t.schedule });
    const a = list.querySelector('#r-a span') ?? list;
    te(a, 'touchstart', 200, 24);
    // Android's WebView sends its own contextmenu on a held touch (a text selection, then touchcancel): the press keeps it
    const cm = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    a.dispatchEvent(cm);
    expect(cm.defaultPrevented).toBe(true);
    t.run();
    te(a, 'touchmove', 200, 130);
    te(a, 'touchend', 200, 130);
    expect(onReorder).toHaveBeenCalledWith('a', undefined);
    // with no press under way, a contextmenu is the page's
    const free = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    a.dispatchEvent(free);
    expect(free.defaultPrevented).toBe(false);
    vi.unstubAllGlobals();
  });

  it('a_keyboard_click_after_a_drag_is_never_swallowed', () => {
    vi.stubGlobal('innerWidth', W);
    const ids = ['a', 'b'];
    const list = queue(ids);
    const t = timer();
    const onClick = vi.fn();
    list.addEventListener('click', onClick);
    bindReorderDrag(list, { host: host(), item: "[data-part~='composer.queue.row']", ids: () => ids, onReorder: () => undefined, schedule: t.schedule });
    const a = list.querySelector('#r-a') ?? list;
    te(a, 'touchstart', 200, 24);
    t.run();
    te(a, 'touchend', 200, 24);
    // no click came from the lift; later, Enter on a control in the queue
    a.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }));
    a.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    expect(onClick).toHaveBeenCalledTimes(1);
    vi.unstubAllGlobals();
  });

  it('move_up_and_move_down_are_the_drags_twins', () => {
    expect(stepBefore(['a', 'b', 'c'], 'b', -1)).toEqual({ before: 'a' });
    expect(stepBefore(['a', 'b', 'c'], 'b', 1)).toEqual({ before: undefined });
    expect(stepBefore(['a', 'b', 'c'], 'a', 1)).toEqual({ before: 'c' });
    expect(stepBefore(['a', 'b', 'c'], 'a', -1)).toBeUndefined();
    expect(stepBefore(['a', 'b', 'c'], 'c', 1)).toBeUndefined();
    expect(reordered(['a', 'b', 'c'], 'a', 'c')).toEqual(['b', 'a', 'c']);
  });
});
