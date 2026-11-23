// The task page's follow state (t3code timelineViewport / opencode auto-scroll-machine / use-stick-to-bottom):
// only the reader's deliberate upward move leaves it; downward scrolls, our own writes, shrink-clamps and layout
// settling never pause it; small upward flicks or scrollbar drags never get yanked back.
import { expect, test } from 'vitest';
import { END_PX, FOLLOWING, PAUSED, awayFromEnd, follow, type FollowEvent, type FollowState } from '../src/app/follow.ts';

const run = (s: FollowState, ...es: FollowEvent[]): FollowState => es.reduce(follow, s);

test('streaming_never_pauses_following_our_own_write_is_caught_up_even_when_the_page_grew_before_its_scroll_event', () => {
  // we pinned the end at 1000; a chunk grew the page to max 1040 before the scroll event for 1000 came
  expect(run(FOLLOWING, { type: 'pinned', top: 1000, max: 1000 }, { type: 'scroll', top: 1000, max: 1040, moved: 0 }).following).toBe(true);
  // two pins coalesced (1050 then 1100), scroll event arrives for 1050 (moving down toward 1100): still following
  expect(run(FOLLOWING, { type: 'pinned', top: 1050, max: 1050 }, { type: 'pinned', top: 1100, max: 1100 }, { type: 'scroll', top: 1050, max: 1140, moved: 50 }).following).toBe(true);
  // pin wrote top=1000 (flight=1000), then turns shrank or grew while our scroll event fired with moved=0: still following
  expect(run(FOLLOWING, { type: 'pinned', top: 1000, max: 1000 }, { type: 'scroll', top: 960, max: 960, moved: 0 }).following).toBe(true);
  // the browser clamping a shrink leaves the view at the end: still following
  expect(run(FOLLOWING, { type: 'scroll', top: 800, max: 800, moved: -40 }).following).toBe(true);
  // macOS rubber-band overscroll settling back to exact max: still following
  expect(run({ following: true, max: 1000 }, { type: 'scroll', top: 1000, max: 1000, moved: -12 }).following).toBe(true);
});

test('a_downward_scroll_while_following_never_pauses_even_when_streaming_grew_past_END_PX', () => {
  // user or script scrolled down (+40) while fast streaming pushed max 80px ahead: never pauses
  expect(run(FOLLOWING, { type: 'scroll', top: 1040, max: 1120, moved: 40 }).following).toBe(true);
});

test('a_pixel_of_layout_settling_while_a_chunk_lands_never_pauses_following', () => {
  // seen on a long task: the view moved 1 px (38455 → 38454) as the page grew 30 px, before the frame's pin
  expect(run(FOLLOWING, { type: 'scroll', top: 38454, max: 38485, moved: -1 }).following).toBe(true);
  // and it never resumes a paused reader far from the end either
  expect(run(PAUSED, { type: 'scroll', top: 500, max: 1000, moved: 1 }).following).toBe(false);
});

test('a_small_upward_scroll_inside_END_PX_stays_paused_instead_of_fighting_the_user', () => {
  // wheel up -8px from the bottom: onWheel sent 'up', then browser fires scroll with dist=8 <= END_PX — MUST stay paused!
  expect(run(FOLLOWING, { type: 'up' }, { type: 'scroll', top: 992, max: 1000, moved: -8 }).following).toBe(false);
  // scrollbar thumb drag up -5px from the bottom (no wheel event): MUST immediately pause instead of yanking back!
  expect(run({ following: true, max: 1000 }, { type: 'scroll', top: 995, max: 1000, moved: -5 }).following).toBe(false);
  // tiny -1px jitter after wheel-up inside END_PX: stays paused
  expect(run(FOLLOWING, { type: 'up' }, { type: 'scroll', top: 999, max: 1000, moved: -1 }).following).toBe(false);
});

test('the_reader_leaves_by_wheel_key_or_scroll_away_and_comes_back_scrolling_down_into_END_PX_or_by_jump_to_latest', () => {
  // a wheel up pauses at once
  expect(run(FOLLOWING, { type: 'up' }).following).toBe(false);
  // a drag or touch away from the end: a scroll that isn't ours and moves upward
  expect(run(FOLLOWING, { type: 'scroll', top: 600, max: 1000, moved: -400 }).following).toBe(false);
  // programmatic pause (e.g. restoring a kept place)
  expect(run(FOLLOWING, { type: 'pause', max: 1000 }).following).toBe(false);
  // a scroll at a stale flight once paused stays paused (the reader happened to land there)
  expect(run(PAUSED, { type: 'pinned', top: 500 }, { type: 'scroll', top: 500, max: 1000, moved: -500 }).following).toBe(false);
  // scrolling DOWN back within END_PX of the end: following again
  expect(run(PAUSED, { type: 'scroll', top: 1000 - END_PX, max: 1000, moved: 300 }).following).toBe(true);
  expect(run(PAUSED, { type: 'scroll', top: 1000 - END_PX - 1, max: 1000, moved: 300 }).following).toBe(false);
  // slow 1 px downward crawl into END_PX re-attaches; 0 px settle while paused does not
  expect(run(PAUSED, { type: 'scroll', top: 1000 - END_PX, max: 1000, moved: 1 }).following).toBe(true);
  expect(run(PAUSED, { type: 'scroll', top: 1000 - END_PX, max: 1000, moved: 0 }).following).toBe(false);
  // Jump to latest, a follow-up, End key
  expect(run(PAUSED, { type: 'end' }).following).toBe(true);
});

test('jump_to_latest_shows_only_while_paused_and_away_from_the_end', () => {
  const box = (scrollTop: number) => ({ scrollTop, scrollHeight: 2000, clientHeight: 800 });
  expect(awayFromEnd(PAUSED, box(400))).toBe(true);
  expect(awayFromEnd(PAUSED, box(1200 - END_PX))).toBe(false);
  // following: never, even a frame behind the end while a chunk lands
  expect(awayFromEnd(FOLLOWING, box(400))).toBe(false);
});
