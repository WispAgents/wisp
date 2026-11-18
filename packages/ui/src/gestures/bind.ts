// The phone apps' gestures on an element (ADR-0065 build step 7, design/mobile.md §A.4): each `bind*` listens on one
// element and returns the function that stops it (a kit component calls it in onCleanup). They decide with
// gesture-math.ts and talk to the device through a GestureHost (haptics, §A.5). The rules every binder keeps:
//   - touch only: a mouse or a pen drag is a text selection, and the browser at any width binds none of these
//     (the components bind them only under a phone look, usePlatform() !== 'web');
//   - a touch starting in a 24 px edge zone is the system's (Back): ignored, never prevented or captured;
//   - a touch starting in a horizontal scroller (code, a diff, a chip row) scrolls it: a sideways gesture ignores it;
//   - controls keep their own touches (buttons, fields, anything `[data-swipe='none']`).
// Sideways gestures (here) use pointer events with the element's `touch-action: pan-y` (the phone sheets), so a
// vertical drag stays the browser's scroll. The vertical ones (bind-vertical.ts: pull, sheet, queue drag) use touch
// events: a pointer stream is cancelled the moment the browser pans, and these must keep the finger.
import { swipePhase, swipeSpeed } from '../shell-list/swipe-math.ts';
import { GESTURE_SLOP, LONG_PRESS_MS, PAGE_SWIPE_LINE, pageSwipeRelease, scrollsX, touchOwner, type TouchOwner } from './gesture-math.ts';
import type { GestureHost } from './host.tsx';
import { pressTimers, type Schedule } from './press-scope.ts';

export type Dir = 'left' | 'right';

/** Controls inside a gesture's element keep their own touches. */
export const OWN_TOUCH = "button, a[href], input, textarea, select, [contenteditable='true'], [contenteditable='plaintext-only'], [data-swipe='none']";
/** The kit's horizontal scrollers, marked so (a box overflowing with overflow-x auto/scroll counts too). */
const SCROLLER = "[data-kit='code-block'], [data-kit='diff'], [data-scroll-x]";

/** Whether `target`, or a box between it and `root`, scrolls sideways (one style read per box, on touch start only). */
export function inScroller(target: EventTarget | null, root: Element): boolean {
  for (let el = target instanceof Element ? target : null; el && el !== root; el = el.parentElement) {
    if (el.matches(SCROLLER)) return true;
    const cs = getComputedStyle(el);
    if (scrollsX({ overflowX: cs.overflowX, scrollWidth: el.scrollWidth, clientWidth: el.clientWidth, marked: false })) return true;
  }
  return false;
}

/** Who a touch at `x` on `target` under `root` belongs to. */
export function ownerOf(x: number, target: EventTarget | null, root: Element): TouchOwner {
  return touchOwner(x, window.innerWidth, inScroller(target, root));
}

export const owns = (target: EventTarget | null): boolean => target instanceof Element && target.closest(OWN_TOUCH) !== null;

export function listen<K extends keyof HTMLElementEventMap>(el: HTMLElement, type: K, fn: (e: HTMLElementEventMap[K]) => void, opts?: AddEventListenerOptions): () => void {
  el.addEventListener(type, fn, opts);
  return () => el.removeEventListener(type, fn, opts);
}

export const all = (offs: readonly (() => void)[]): (() => void) => () => {
  for (const off of offs) off();
};

// -------------------------------------------------------------------------------------------------------------------
// A page swipe: the task head (next / previous task) and the task body (the side panel, leftward only).

export interface PageSwipeOptions {
  readonly host: GestureHost;
  /** whether that way does anything (the body: only left; the head: a next / a previous task exists) */
  readonly can: (dir: Dir) => boolean;
  readonly onSwipe: (dir: Dir) => void;
  /** how far the page follows, in px (0 when it springs back): the shared-axis / side-sheet preview */
  readonly onDrag?: (dx: number) => void;
}

export function bindPageSwipe(el: HTMLElement, o: PageSwipeOptions): () => void {
  let tr: { x: number; y: number; mode: 'pending' | 'swipe'; armed: boolean; samples: { x: number; t: number }[] } | undefined;
  const end = (): void => {
    if (tr?.mode === 'swipe') o.onDrag?.(0);
    tr = undefined;
  };
  return all([
    listen(el, 'pointerdown', (e) => {
      tr = undefined;
      if (e.pointerType !== 'touch' || !e.isPrimary || owns(e.target)) return;
      if (ownerOf(e.clientX, e.target, el) !== 'gesture') return;
      tr = { x: e.clientX, y: e.clientY, mode: 'pending', armed: false, samples: [{ x: e.clientX, t: e.timeStamp }] };
    }),
    listen(el, 'pointermove', (e) => {
      if (!tr) return;
      const dx = e.clientX - tr.x;
      if (tr.mode === 'pending') {
        const phase = swipePhase(dx, e.clientY - tr.y);
        if (phase === 'scroll' || (phase === 'swipe' && !o.can(dx < 0 ? 'left' : 'right'))) return void (tr = undefined);
        if (phase !== 'swipe') return;
        tr.mode = 'swipe';
        el.setPointerCapture?.(e.pointerId);
      }
      tr.samples.push({ x: e.clientX, t: e.timeStamp });
      if (tr.samples.length > 12) tr.samples.shift();
      o.onDrag?.(dx);
      const armed = Math.abs(dx) >= PAGE_SWIPE_LINE;
      if (armed !== tr.armed) {
        tr.armed = armed;
        if (armed) o.host.haptic('select');
      }
    }),
    listen(el, 'pointerup', (e) => {
      const t = tr;
      end();
      if (t?.mode !== 'swipe') return;
      const dx = e.clientX - t.x;
      const dir: Dir = dx < 0 ? 'left' : 'right';
      if (o.can(dir) && pageSwipeRelease(dx, swipeSpeed(t.samples)) === 'commit') o.onSwipe(dir);
    }),
    listen(el, 'pointercancel', end),
  ]);
}

// -------------------------------------------------------------------------------------------------------------------
// A long-press: a press held still for LONG_PRESS_MS. The row's menu (Android: the bottom sheet of its actions with
// Select first; iOS: the context menu with a preview); a block's Copy / Quote / Share. The tap haptic says it opened.

export interface LongPressOptions {
  readonly host: GestureHost;
  /** `at`: where the finger was (a menu opens beside it). Additive (QA r8 #2). */
  readonly onLongPress: (at: { readonly x: number; readonly y: number }) => void;
  /** the timer (tests pass a fake one) */
  readonly schedule?: Schedule;
  /** run `onLongPress` once the finger is up (a menu opened under the finger would take its lift for a press outside
   *  and close): at once if it already is, else on its touchend (or pointerup, a stream with no touch events).
   *  Additive (QA r7, an answer's menu on Android). */
  readonly onLift?: boolean;
}

/** how soon after the lift the browser's own click comes; a later click is a new press (QA r9: a hold with no click of
 *  its own, Android's or a script's, ate the next tap on the answer) */
const TRAILING_CLICK_MS = 350;

/** the lift events of a finger on the screen, read on the window (it may lift anywhere) */
const LIFTS = ['touchend', 'touchcancel', 'pointerup'] as const;

/** A finger's press, for a long-press: whether it is still down and whether it moved; `whenUp` runs a function on the
 *  frame after it lifts (at once if it already has). Android Chrome ends its pointer stream (pointercancel) as it takes
 *  a long touch for itself, the finger still down until touchend. */
function fingerPress() {
  let held: { moved: boolean; down: boolean } | undefined;
  let waiting: (() => void) | undefined;
  let liftedAt = Number.NEGATIVE_INFINITY;
  const lifted = (): void => {
    for (const k of LIFTS) removeEventListener(k, lifted, true);
    liftedAt = performance.now();
    if (held) held.down = false;
    const run = waiting;
    waiting = undefined;
    if (run) requestAnimationFrame(run);
  };
  return {
    start: (): void => {
      held = { moved: false, down: true };
      for (const k of LIFTS) addEventListener(k, lifted, true);
    },
    reset: (): void => {
      held = undefined;
      waiting = undefined;
    },
    moved: (): void => void (held && (held.moved = true)),
    /** a click this soon after the finger lifted is the press's own (the browser's, after touchend) */
    trailing: (): boolean => performance.now() - liftedAt < TRAILING_CLICK_MS,
    /** held down and still: Android's contextmenu after it took the touch is this press's */
    still: (): boolean => held?.down === true && !held.moved,
    whenUp: (run: () => void): void => {
      if (held?.down) waiting = run;
      else requestAnimationFrame(run);
    },
    dispose: (): void => {
      for (const k of LIFTS) removeEventListener(k, lifted, true);
    },
  };
}

export function bindLongPress(el: HTMLElement, o: LongPressOptions): () => void {
  const timers = pressTimers();
  const schedule = o.schedule ?? timers.schedule;
  let st: { x: number; y: number; cancel: () => void } | undefined;
  /** the press opened the menu: the click (and Android's own contextmenu) that follow belong to it */
  let fired = false;
  const finger = fingerPress();
  const stop = (): void => {
    st?.cancel();
    st = undefined;
  };
  const fire = (at: { readonly x: number; readonly y: number }): void => {
    stop();
    fired = true;
    o.host.haptic('tap');
    if (o.onLift) finger.whenUp(() => o.onLongPress(at));
    else o.onLongPress(at);
  };
  return all([
    timers.dispose,
    finger.dispose,
    listen(el, 'pointerdown', (e) => {
      stop();
      fired = false;
      finger.reset();
      if (e.pointerType !== 'touch' || !e.isPrimary || owns(e.target)) return;
      if (ownerOf(e.clientX, e.target, el) === 'system') return;
      st = { x: e.clientX, y: e.clientY, cancel: schedule(() => fire({ x: e.clientX, y: e.clientY }), LONG_PRESS_MS) };
      finger.start();
    }),
    listen(el, 'pointermove', (e) => {
      if (st && Math.hypot(e.clientX - st.x, e.clientY - st.y) > GESTURE_SLOP) {
        stop();
        finger.moved();
      }
    }),
    listen(el, 'pointerup', stop),
    listen(el, 'pointercancel', stop), // the browser took the touch (a scroll, its own long-press): the finger may stay
    // a click from a key (Enter, a screen reader) is never the press's
    listen(el, 'keydown', () => void (fired = false), { capture: true }),
    // Android's own contextmenu on a long touch is this gesture, run once (also after it ended the pointer stream)
    listen(el, 'contextmenu', (e) => {
      if (!st && !fired && !finger.still()) return;
      e.preventDefault();
      if (!fired) fire({ x: e.clientX, y: e.clientY });
    }),
    listen(
      el,
      'click',
      (e) => {
        if (!fired) return;
        fired = false;
        if (!finger.trailing()) return;
        e.preventDefault();
        e.stopPropagation();
      },
      { capture: true },
    ),
  ]);
}
