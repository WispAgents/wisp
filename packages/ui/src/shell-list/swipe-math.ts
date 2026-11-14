// The decisions of a sideways swipe on a touch row (v1 web/js/features/swipe.js, design/mobile.md),
// pure so the thresholds have tests: when a drag is a swipe or a scroll, how far the row follows,
// when letting go commits.

/** a touch starting this close to the screen's edge is the system's Back gesture */
export const SWIPE_EDGE = 24;
/** movement before the drag is a swipe or a scroll */
export const SWIPE_SLOP = 10;
/** past this share of the row's width, letting go commits */
export const SWIPE_COMMIT = 0.34;
/** a flick commits from a shorter pull: this fast (px/ms) over the last `window` ms, at least `min` px out */
export const SWIPE_FLICK = { speed: 0.5, min: 56, window: 100 } as const;
/** a side with no action only gives this much, at a fifth of the pull */
export const SWIPE_GIVE = 28;

export type SwipePhase = 'pending' | 'swipe' | 'scroll';

export function swipePhase(dx: number, dy: number): SwipePhase {
  const ax = Math.abs(dx);
  const ay = Math.abs(dy);
  if (ay > SWIPE_SLOP && ay >= ax) return 'scroll';
  if (ax >= SWIPE_SLOP && ax >= 1.5 * ay) return 'swipe';
  return 'pending';
}

export const swipeStarts = (x: number, screenW: number): boolean => x > SWIPE_EDGE && x < screenW - SWIPE_EDGE;

/** How far the row follows the finger: all the way when that side has an action, a little when not. */
export const swipeOffset = (dx: number, can: boolean): number => (can ? dx : Math.sign(dx) * Math.min(SWIPE_GIVE, Math.abs(dx) * 0.2));

export const swipeArmed = (dx: number, width: number, can: boolean): boolean => can && Math.abs(dx) > width * SWIPE_COMMIT;

/** Letting go commits when armed, or on a flick the same way as the pull. */
export function swipeCommits(dx: number, width: number, can: boolean, speed: number): boolean {
  if (!can) return false;
  if (swipeArmed(dx, width, can)) return true;
  return Math.abs(speed) >= SWIPE_FLICK.speed && Math.abs(dx) >= SWIPE_FLICK.min && Math.sign(speed) === Math.sign(dx);
}

/** px/ms over the samples of the last `window` ms (oldest first). */
export function swipeSpeed(samples: readonly { readonly x: number; readonly t: number }[], window: number = SWIPE_FLICK.window): number {
  const last = samples[samples.length - 1];
  if (!last) return 0;
  const first = samples.find((s) => last.t - s.t <= window) ?? last;
  const dt = last.t - first.t;
  return dt > 0 ? (last.x - first.x) / dt : 0;
}

// The phone apps' two swipe shapes (ADR-0065 build step 4, design/mobile.md §A.4): the browser and Android
// draw a band (Material 3: tonal, icon leading; v1's third of the row commits); iOS reveals Mail's action
// from the edge: let go past half a button and the row rests open on it, past half the row (a full swipe)
// it runs. Both keep v1's rules: a side with no action only gives a little; the edge zone is the system's.

/** `band`: the browser and Android; `reveal`: iOS (Mail). */
export type SwipeStyle = 'band' | 'reveal';
/** iOS: an action's width when the row rests open (UIContextualAction; tokens/platform/ios.json --swipe-action-w) */
export const SWIPE_ACTION_W = 74;
/** iOS: past this share of the row a release runs the action (Mail's full swipe; ios.json --swipe-line) */
export const SWIPE_FULL = 0.5;

/** the share of the row past which letting go runs the action */
export const swipeLine = (style: SwipeStyle): number => (style === 'reveal' ? SWIPE_FULL : SWIPE_COMMIT);

/** Past the line: the band fills, the haptic ticks. */
export const swipeArmedFor = (style: SwipeStyle, dx: number, width: number, can: boolean): boolean => can && Math.abs(dx) > width * swipeLine(style);

/** How warm the band is: 0 at rest, 1 on the line (it may pass 1), 0 on a side with no action. */
export function swipeProgress(style: SwipeStyle, dx: number, width: number, can: boolean): number {
  if (!can || width <= 0) return 0;
  return Math.abs(dx) / (width * swipeLine(style));
}

export type SwipeRelease = 'commit' | 'open' | 'close';

/** What letting go does: run the action, rest open on it (iOS only), or spring back. */
export function swipeRelease(style: SwipeStyle, dx: number, width: number, can: boolean, speed: number): SwipeRelease {
  if (style === 'band') return swipeCommits(dx, width, can, speed) ? 'commit' : 'close';
  if (!can) return 'close';
  if (swipeArmedFor(style, dx, width, can)) return 'commit';
  // a flick back toward the start closes, whatever the pull
  if (Math.abs(speed) >= SWIPE_FLICK.speed && Math.sign(speed) !== Math.sign(dx)) return 'close';
  const flickOut = Math.abs(speed) >= SWIPE_FLICK.speed && Math.abs(dx) >= SWIPE_SLOP;
  return flickOut || Math.abs(dx) >= SWIPE_ACTION_W / 2 ? 'open' : 'close';
}
