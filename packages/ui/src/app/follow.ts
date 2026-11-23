// Whether a task's page follows its end as output streams in (the model t3code's timelineViewport and opencode's
// auto-scroll machine share): following pins the end every frame the page grows; only the reader's own move leaves it
// (a wheel or key up, a drag or touch away from the end, selecting text while it streams); coming back near the end, a
// follow-up or Jump to latest follows again. The page's own writes to the end (`pinned`) are told apart from the
// reader's scrolls by where they left the view (`flight`): content that grew between the write and its scroll event
// is caught up, never read as the reader leaving. Pure: the hold-view feeds it the numbers.

/** How near the end (px) counts as at it: a reader back within it follows again. */
export const END_PX = 16;
/** A scroll this small is the layout settling, not the reader moving. */
export const NUDGE_PX = 2;

export interface FollowState {
  readonly following: boolean;
  /** the scrollTop our last write to the end left, until its scroll event comes */
  readonly flight?: number | undefined;
}

export type FollowEvent =
  /** a scroll event: where the view is (`top`), how far it can go (`max`, scrollHeight − clientHeight) and how far it
   *  moved since the last one (`moved`, px) */
  | { readonly type: 'scroll'; readonly top: number; readonly max: number; readonly moved: number }
  /** the reader moved toward older output: wheel or key up, selecting text while it streams */
  | { readonly type: 'up' }
  /** follow from here: Jump to latest, a follow-up sent */
  | { readonly type: 'end' }
  /** the page wrote the end itself, leaving the view at `top` */
  | { readonly type: 'pinned'; readonly top: number };

export const FOLLOWING: FollowState = { following: true };
export const PAUSED: FollowState = { following: false };

export function follow(s: FollowState, e: FollowEvent): FollowState {
  switch (e.type) {
    case 'scroll':
      if (e.max - e.top <= END_PX) return FOLLOWING;
      // our own write, and the page grew again before its scroll event: still following (the next frame pins)
      if (s.following && s.flight !== undefined && Math.abs(e.top - s.flight) <= 1) return FOLLOWING;
      // a pixel or two is the layout's (rounding, a chunk landing a frame before the pin), never the reader: only a move
      // (a drag, a touch, a jump to a kept place) leaves the end; a wheel or key up already said so on its own
      if (Math.abs(e.moved) <= NUDGE_PX) return s.following ? FOLLOWING : s;
      return PAUSED;
    case 'up':
      return PAUSED;
    case 'end':
      return FOLLOWING;
    case 'pinned':
      return s.following ? { following: true, flight: e.top } : s;
  }
}

/** Jump to latest shows while the reader is away: not following, and further than END_PX from the end. */
export const awayFromEnd = (s: FollowState, box: { readonly scrollTop: number; readonly scrollHeight: number; readonly clientHeight: number }): boolean =>
  !s.following && box.scrollHeight - box.clientHeight - box.scrollTop > END_PX;
