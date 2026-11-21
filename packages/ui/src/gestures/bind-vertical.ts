// The phone apps' vertical gestures (ADR-0065 build step 7, design/mobile.md §A.4): pull to refresh, a sheet dragged
// down, the queue's drag to reorder. Touch events, not pointer events: a pointer stream is cancelled the moment the
// browser pans, and these keep the finger while the list sits at its top. The rules of bind.ts hold here too (touch
// only, the system's edges, controls keep their touches); each `bind*` returns its stop.
import { swipePhase, swipeSpeed } from '../shell-list/swipe-math.ts';
import { GESTURE_SLOP, LONG_PRESS_MS, pullProgress, reorderSlot, sheetCloses, sheetProgress, slotBefore } from './gesture-math.ts';
import { all, listen, ownerOf, owns } from './bind.ts';
import type { GestureHost } from './host.tsx';
import { pressTimers, type Schedule } from './press-scope.ts';

const pointOf = (e: TouchEvent): { x: number; y: number } | undefined => {
  const t = e.touches[0] ?? e.changedTouches[0];
  return t ? { x: t.clientX, y: t.clientY } : undefined;
};

// -------------------------------------------------------------------------------------------------------------------
// Pull to refresh: a list at its top, pulled down. Letting go past the line reconnects every computer now (the
// caller's `onRefresh` skips the backoff; clients never poll). `onProgress` feeds PullRefresh's look.

export interface PullOptions {
  readonly host: GestureHost;
  readonly onRefresh: () => void;
  readonly onProgress: (progress: number) => void;
  /** a refresh is already on its way: a pull does nothing */
  readonly refreshing: () => boolean;
}

export function bindPull(scroller: HTMLElement, o: PullOptions): () => void {
  let tr: { x: number; y: number; mode: 'pending' | 'pull'; armed: boolean; p: number } | undefined;
  const end = (): void => {
    if (tr?.mode === 'pull') o.onProgress(0);
    tr = undefined;
  };
  return all([
    listen(
      scroller,
      'touchstart',
      (e) => {
        tr = undefined;
        const at = pointOf(e);
        if (!at || e.touches.length > 1 || o.refreshing() || scroller.scrollTop > 0) return;
        if (ownerOf(at.x, e.target, scroller) === 'system') return;
        tr = { ...at, mode: 'pending', armed: false, p: 0 };
      },
      { passive: true },
    ),
    listen(
      scroller,
      'touchmove',
      (e) => {
        const at = pointOf(e);
        if (!tr || !at) return;
        const dy = at.y - tr.y;
        if (tr.mode === 'pending') {
          const phase = swipePhase(at.x - tr.x, dy);
          if (phase === 'swipe' || (phase === 'scroll' && dy < 0) || scroller.scrollTop > 0) return void (tr = undefined);
          if (phase !== 'scroll') return;
          tr.mode = 'pull';
        }
        if (e.cancelable) e.preventDefault();
        tr.p = pullProgress(dy);
        o.onProgress(tr.p);
        const armed = tr.p >= 1;
        if (armed !== tr.armed) {
          tr.armed = armed;
          if (armed) o.host.haptic('select');
        }
      },
      { passive: false },
    ),
    listen(scroller, 'touchend', () => {
      const t = tr;
      end();
      if (t?.mode === 'pull' && t.p >= 1) o.onRefresh();
    }),
    listen(scroller, 'touchcancel', end),
  ]);
}

// -------------------------------------------------------------------------------------------------------------------
// A sheet dragged down: past SHEET_CLOSE of its height (or a flick) it closes; the caller keeps the draft. The drag
// starts only while the sheet's own content sits at its top, so scrolling the sheet stays a scroll.

export interface SheetDragOptions {
  readonly host: GestureHost;
  readonly onClose: () => void;
  /** how far the sheet follows (px, 0 when it springs back) and how near closing it is (0…1 at the line) */
  readonly onDrag: (dy: number, progress: number) => void;
  /** the sheet's scrolling content, when it has one */
  readonly content?: () => HTMLElement | undefined;
  /** an iOS sheet below its largest detent: a drag up past the line (or a flick up) grows it (step 5) */
  readonly onLift?: () => void;
  /** whether a drag up may grow it now (asked as the drag starts; at its largest a drag up scrolls the content) */
  readonly canLift?: () => boolean;
}

export function bindSheetDrag(sheet: HTMLElement, o: SheetDragOptions): () => void {
  let tr: { x: number; y: number; h: number; mode: 'pending' | 'drag' | 'lift'; armed: boolean; samples: { x: number; t: number }[] } | undefined;
  const end = (): void => {
    if (tr && tr.mode !== 'pending') o.onDrag(0, 0);
    tr = undefined;
  };
  return all([
    listen(
      sheet,
      'touchstart',
      (e) => {
        tr = undefined;
        const at = pointOf(e);
        if (!at || e.touches.length > 1 || owns(e.target) || (o.content?.()?.scrollTop ?? 0) > 0) return;
        tr = { ...at, h: sheet.getBoundingClientRect().height, mode: 'pending', armed: false, samples: [{ x: at.y, t: e.timeStamp }] };
      },
      { passive: true },
    ),
    listen(
      sheet,
      'touchmove',
      (e) => {
        const at = pointOf(e);
        if (!tr || !at) return;
        const dy = at.y - tr.y;
        if (tr.mode === 'pending') {
          const phase = swipePhase(at.x - tr.x, dy);
          if (phase === 'swipe' || (phase === 'scroll' && dy < 0 && !(o.onLift && (o.canLift?.() ?? true)))) return void (tr = undefined);
          if (phase !== 'scroll') return;
          tr.mode = dy < 0 ? 'lift' : 'drag';
        }
        if (e.cancelable) e.preventDefault();
        // swipeSpeed reads `x`: here it carries the vertical position
        tr.samples.push({ x: at.y, t: e.timeStamp });
        if (tr.samples.length > 12) tr.samples.shift();
        if (tr.mode === 'lift') return void o.onDrag(Math.min(0, dy), 0);
        const p = sheetProgress(dy, tr.h);
        o.onDrag(Math.max(0, dy), p);
        const armed = p >= 1;
        if (armed !== tr.armed) {
          tr.armed = armed;
          if (armed) o.host.haptic('select');
        }
      },
      { passive: false },
    ),
    listen(sheet, 'touchend', (e) => {
      const t = tr;
      const at = pointOf(e);
      end();
      if (t?.mode === 'drag' && at && sheetCloses(at.y - t.y, t.h, swipeSpeed(t.samples))) o.onClose();
      else if (t?.mode === 'lift' && at && sheetCloses(t.y - at.y, t.h, -swipeSpeed(t.samples))) o.onLift?.();
    }),
    listen(sheet, 'touchcancel', end),
  ]);
}

// -------------------------------------------------------------------------------------------------------------------
// The queue's drag to reorder: long-press an item (the tap haptic), then drag it up or down; each slot it crosses
// ticks (select); letting go says where it went with `onReorder(id, before)`, core/reorder.ts's shape. Items are the
// elements matching `item`, in the same order as `ids()` (a keyed list keeps them so); the dragged one gets
// `data-dragging` and `--k-drag-y` (how far it follows the finger), and the click that ends the press is swallowed.

export interface ReorderDragOptions {
  readonly host: GestureHost;
  /** the items' selector inside the list */
  readonly item: string;
  readonly ids: () => readonly string[];
  readonly onReorder: (id: string, before: string | undefined) => void;
  readonly schedule?: Schedule;
}

export function bindReorderDrag(list: HTMLElement, o: ReorderDragOptions): () => void {
  const timers = pressTimers();
  const schedule = o.schedule ?? timers.schedule;
  const items = (): HTMLElement[] => [...list.querySelectorAll<HTMLElement>(o.item)];
  let st: { item: HTMLElement; from: number; x: number; y: number; cancel: () => void; lifted: boolean; mids: number[]; slot: number } | undefined;
  let swallow = false;
  const drop = (): void => {
    if (!st) return;
    st.cancel();
    st.item.removeAttribute('data-dragging');
    st.item.style.removeProperty('--k-drag-y');
    st = undefined;
  };
  const lift = (): void => {
    if (!st) return;
    const s = st;
    s.lifted = true;
    // one layout read per item, at the lift: the others' centres, in order
    s.mids = items()
      .filter((el) => el !== s.item)
      .map((el) => {
        const r = el.getBoundingClientRect();
        return r.top + r.height / 2;
      });
    s.slot = s.from;
    s.item.setAttribute('data-dragging', '');
    o.host.haptic('tap');
  };
  return all([
    timers.dispose,
    listen(
      list,
      'touchstart',
      (e) => {
        drop();
        swallow = false;
        const at = pointOf(e);
        const item = e.target instanceof Element ? e.target.closest<HTMLElement>(o.item) : null;
        if (!at || !item || !list.contains(item) || e.touches.length > 1 || owns(e.target) || ownerOf(at.x, e.target, list) === 'system') return;
        const from = items().indexOf(item);
        // every item is in `ids()`'s order, or none moves (an item being edited stands in for its row)
        if (from < 0 || items().length !== o.ids().length || o.ids().length < 2) return;
        st = { item, from, ...at, cancel: schedule(lift, LONG_PRESS_MS), lifted: false, mids: [], slot: from };
      },
      { passive: true },
    ),
    listen(
      list,
      'touchmove',
      (e) => {
        const at = pointOf(e);
        if (!st || !at) return;
        if (!st.lifted) {
          // moving before the press is long is a scroll
          if (Math.hypot(at.x - st.x, at.y - st.y) > GESTURE_SLOP) drop();
          return;
        }
        if (e.cancelable) e.preventDefault();
        st.item.style.setProperty('--k-drag-y', `${at.y - st.y}px`);
        const slot = reorderSlot(st.mids, at.y);
        if (slot !== st.slot) {
          st.slot = slot;
          o.host.haptic('select');
        }
      },
      { passive: false },
    ),
    listen(list, 'touchend', () => {
      const s = st;
      drop();
      if (!s?.lifted) return;
      swallow = true;
      const ids = o.ids();
      const id = ids[s.from];
      if (id !== undefined && s.slot !== s.from) o.onReorder(id, slotBefore(ids.filter((x) => x !== id), s.slot));
    }),
    listen(list, 'touchcancel', drop),
    // Android's WebView answers a held touch with its own contextmenu (a text selection, then touchcancel): the
    // press under way is this gesture's
    listen(list, 'contextmenu', (e) => {
      if (st) e.preventDefault();
    }),
    // a click from a key (Enter, a screen reader) is never the lift's
    listen(list, 'keydown', () => void (swallow = false), { capture: true }),
    listen(
      list,
      'click',
      (e) => {
        if (!swallow) return;
        swallow = false;
        e.preventDefault();
        e.stopPropagation();
      },
      { capture: true },
    ),
  ]);
}
