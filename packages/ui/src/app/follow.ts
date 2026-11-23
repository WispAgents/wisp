// Whether a task's page follows its end as output streams in (the model t3code's timelineViewport, opencode's
// auto-scroll machine, and use-stick-to-bottom share): following pins the end every frame the page grows; ONLY the
// reader's deliberate upward move leaves it (wheel/key/touch/scrollbar-drag up, or selecting text while it streams);
// scrolling down into the bottom zone (END_PX), pressing End, sending a follow-up or clicking Jump to latest follows
// again. Downward scrolls, our own writes (`flight`), browser shrink-clamps and 1–2 px of layout settling NEVER pause
// following; small upward moves within END_PX NEVER re-engage it (so the reader never fights the scroller). Pure.

/** How near the end (px) counts as at it when scrolling down (covers the dock's fade + padding and a line of text). */
export const END_PX = 48;
/** A scroll this small is rounding or layout settling, not the reader moving. */
export const NUDGE_PX = 2;

export interface FollowState {
  readonly following: boolean;
  /** the scrollTop our last write to the end aimed for, until its scroll event catches up */
  readonly flight?: number | undefined;
  /** last observed max (scrollHeight - clientHeight), to recognize browser shrink-clamps */
  readonly max?: number | undefined;
}

export type FollowEvent =
  /** a scroll event: where the view is (`top`), how far it can go (`max`, scrollHeight − clientHeight) and how far it
   *  moved since the last one (`moved`, px; negative = up toward older output, positive = down toward the end) */
  | { readonly type: 'scroll'; readonly top: number; readonly max: number; readonly moved: number }
  /** the reader moved toward older output: wheel/key/touch up, selecting text while it streams */
  | { readonly type: 'up' }
  /** programmatic placement away from the end (restoring a saved reading place, Jump to top, opening a fold) */
  | { readonly type: 'pause'; readonly max?: number | undefined }
  /** follow from here: Jump to latest, a follow-up sent, initial Chat landing, End key */
  | { readonly type: 'end'; readonly top?: number | undefined; readonly max?: number | undefined }
  /** the page wrote the end itself, leaving the view at `top` */
  | { readonly type: 'pinned'; readonly top: number; readonly max?: number | undefined };

export const FOLLOWING: FollowState = { following: true };
export const PAUSED: FollowState = { following: false };

const stateOf = (following: boolean, max?: number, flight?: number): FollowState => ({
  following,
  ...(flight !== undefined && { flight }),
  ...(max !== undefined && { max }),
});

export function follow(s: FollowState, e: FollowEvent): FollowState {
  switch (e.type) {
    case 'scroll': {
      const dist = Math.max(0, e.max - e.top);
      const shrank = s.max !== undefined && e.max < s.max - 0.5;
      // 1. Our own pin in flight: reaching it (or progressing downward toward it across coalesced frames) keeps following;
      //    and if the page grew while our pin was in flight (moved === 0 after hold-view updated lastTop on write),
      //    any scroll event at or below the new max that didn't move upward is still our pin catching up
      if (s.following && s.flight !== undefined) {
        if (Math.abs(e.top - s.flight) <= NUDGE_PX) return stateOf(true, e.max);
        if (e.moved >= 0 && e.top < s.flight) return stateOf(true, e.max, s.flight);
        if (e.moved >= -NUDGE_PX) return stateOf(true, e.max);
      }
      // 2. Upward move (away from the end): pauses immediately — even inside END_PX so small flicks or scrollbar drags
      //    are never yanked back — except a browser shrink-clamp / macOS rubber-band settle at the exact bottom
      if (e.moved < -NUDGE_PX) {
        if (dist <= 1 || (shrank && dist <= NUDGE_PX)) return stateOf(s.following, e.max);
        return stateOf(false, e.max);
      }
      // 3. Downward move (toward the end): never pauses following; while paused, re-attaches inside END_PX
      if (e.moved > NUDGE_PX) {
        return stateOf(s.following || dist <= END_PX, e.max);
      }
      // 4. Tiny move (<= NUDGE_PX: rounding, layout settling, or slow 1 px crawl):
      //    never pauses following; while paused, a slow downward crawl into END_PX re-attaches, upward/zero stays paused
      if (s.following) return stateOf(true, e.max);
      if (e.moved > 0 && dist <= END_PX) return stateOf(true, e.max);
      return stateOf(false, e.max);
    }
    case 'up':
      return stateOf(false, s.max);
    case 'pause':
      return stateOf(false, e.max ?? s.max);
    case 'end':
      return stateOf(true, e.max ?? s.max, e.top);
    case 'pinned':
      return s.following ? stateOf(true, e.max ?? s.max, e.top) : s;
  }
}

/** Jump to latest shows while the reader is away: not following, and further than END_PX from the end. */
export const awayFromEnd = (s: FollowState, box: { readonly scrollTop: number; readonly scrollHeight: number; readonly clientHeight: number }): boolean =>
  !s.following && box.scrollHeight - box.clientHeight - box.scrollTop > END_PX;
