// The decisions behind the phone apps' gestures (ADR-0065 build step 7, design/mobile.md §A.4), pure so each rule
// has a test: who owns a touch (the system's edge, a scroller, the gesture), when a pull or a drag commits, which
// slot a dragged queue item lands in. No DOM, no clock: the binders (bind.ts) measure and feed these.
import { SWIPE_EDGE, SWIPE_FLICK, SWIPE_SLOP } from '../shell-list/swipe-math.ts';

export { SWIPE_EDGE as GESTURE_EDGE, SWIPE_SLOP as GESTURE_SLOP } from '../shell-list/swipe-math.ts';

/** A press held this long without moving is a long-press (Android ViewConfiguration's 400–500 ms; iOS 0.5 s). */
export const LONG_PRESS_MS = 500;
/** A page swipe (task head, task body) commits past this many px, or on a flick. */
export const PAGE_SWIPE_LINE = 72;
/** A pull to refresh arms this far down (after the resistance below); M3 and UIRefreshControl both sit near 64–80. */
export const PULL_LINE = 72;
/** The list follows a pull at this share of the finger (both platforms resist). */
export const PULL_RESIST = 0.5;
/** A sheet dragged down past this share of its height closes on release (or on a downward flick). */
export const SHEET_CLOSE = 0.3;

export type ScreenEdge = 'left' | 'right';

/** The edge zone a touch starts in (24 px, both sides): those touches are the system's (Back), never a gesture's. */
export function edgeOf(x: number, width: number): ScreenEdge | undefined {
  if (x <= SWIPE_EDGE) return 'left';
  if (x >= width - SWIPE_EDGE) return 'right';
  return undefined;
}

/** Whether an edge zone is Back on that platform: Android either edge (its gesture navigation), iOS the left. */
export const edgeIsBack = (edge: ScreenEdge, look: 'android' | 'ios'): boolean => look === 'android' || edge === 'left';

/** What the start of a touch says about who gets it. */
export type TouchOwner = 'system' | 'scroller' | 'gesture';

/** The system's edge first, then a horizontal scroller under the finger (code, a diff, a chip row), else the gesture. */
export function touchOwner(x: number, width: number, inScroller: boolean): TouchOwner {
  if (edgeOf(x, width)) return 'system';
  if (inScroller) return 'scroller';
  return 'gesture';
}

export interface ScrollBox {
  /** CSS overflow-x */
  readonly overflowX: string;
  readonly scrollWidth: number;
  readonly clientWidth: number;
  /** marked as a horizontal scroller by the kit (`[data-kit=code-block|diff]`, `[data-scroll-x]`) */
  readonly marked: boolean;
}

/** A box that scrolls sideways: marked so, or overflowing with overflow-x auto/scroll. */
export const scrollsX = (b: ScrollBox): boolean => b.marked || ((b.overflowX === 'auto' || b.overflowX === 'scroll') && b.scrollWidth > b.clientWidth);

export type PageSwipeRelease = 'commit' | 'cancel';

/** A page swipe's release: past the line, or a flick at least the slop out the same way. */
export function pageSwipeRelease(dx: number, speed: number): PageSwipeRelease {
  if (Math.abs(dx) >= PAGE_SWIPE_LINE) return 'commit';
  const flick = Math.abs(speed) >= SWIPE_FLICK.speed && Math.abs(dx) >= SWIPE_SLOP * 2 && Math.sign(speed) === Math.sign(dx);
  return flick ? 'commit' : 'cancel';
}

/** How far a pull has come: 0 at rest, 1 on the line (it may pass 1); an upward drag is no pull. */
export const pullProgress = (dy: number): number => (dy <= 0 ? 0 : (dy * PULL_RESIST) / PULL_LINE);

/** A sheet dragged down `dy` of its `height`: closes past the share, or on a downward flick. */
export function sheetCloses(dy: number, height: number, speed: number): boolean {
  if (dy <= 0 || height <= 0) return false;
  return dy >= height * SHEET_CLOSE || (speed >= SWIPE_FLICK.speed && dy >= SWIPE_SLOP * 2);
}

/** A dragged sheet's progress toward closing: 0…1 at the line. */
export const sheetProgress = (dy: number, height: number): number => (dy <= 0 || height <= 0 ? 0 : Math.min(dy / (height * SHEET_CLOSE), 1.5));

/**
 * Where a dragged item lands: `mids` are the other items' vertical centres in order (the dragged one left out);
 * the item goes before the first whose centre is below `y`. Returns that index in the others (mids.length: last).
 */
export function reorderSlot(mids: readonly number[], y: number): number {
  const i = mids.findIndex((m) => y < m);
  return i < 0 ? mids.length : i;
}

/** The `before` of `onReorder(id, before)` for slot `slot` among the other ids (undefined: the end). */
export const slotBefore = (others: readonly string[], slot: number): string | undefined => others[slot];

/** One step up (-1) or down (+1) as `onReorder`'s `before`: Move up / Move down, the drag's accessible twins. */
export function stepBefore(ids: readonly string[], id: string, step: -1 | 1): { readonly before: string | undefined } | undefined {
  const i = ids.indexOf(id);
  const j = i + step;
  if (i < 0 || j < 0 || j >= ids.length) return undefined;
  const others = ids.filter((x) => x !== id);
  // the others without `id`: going up, it sits before the one now at j; going down, before the one after it
  return { before: others[j] };
}
