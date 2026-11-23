// The task page's follow state (t3code timelineViewport / opencode auto-scroll-machine): only the reader leaves it.
import { expect, test } from 'vitest';
import { END_PX, FOLLOWING, PAUSED, awayFromEnd, follow, type FollowEvent, type FollowState } from '../src/app/follow.ts';

const run = (s: FollowState, ...es: FollowEvent[]): FollowState => es.reduce(follow, s);

test('streaming_never_pauses_following_our_own_write_is_caught_up_even_when_the_page_grew_before_its_scroll_event', () => {
  // we pinned the end at 1000; a chunk grew the page to max 1040 before the scroll event for 1000 came
  expect(run(FOLLOWING, { type: 'pinned', top: 1000 }, { type: 'scroll', top: 1000, max: 1040, moved: 0 }).following).toBe(true);
  // the browser clamping a shrink leaves the view at the end: still following
  expect(run(FOLLOWING, { type: 'scroll', top: 800, max: 800, moved: -40 }).following).toBe(true);
});

test('a_pixel_of_layout_settling_while_a_chunk_lands_never_pauses_following', () => {
  // seen on a long task: the view moved 1 px (38455 → 38454) as the page grew 30 px, before the frame's pin
  expect(run(FOLLOWING, { type: 'scroll', top: 38454, max: 38485, moved: -1 }).following).toBe(true);
  // and it never resumes a paused reader either
  expect(run(PAUSED, { type: 'scroll', top: 500, max: 1000, moved: 1 }).following).toBe(false);
});

test('the_reader_leaves_by_wheel_key_or_scroll_away_and_comes_back_near_the_end_or_by_jump_to_latest', () => {
  // a wheel up pauses at once, even a small one
  expect(run(FOLLOWING, { type: 'up' }).following).toBe(false);
  // a drag or touch away from the end: a scroll that isn't ours and isn't at the end
  expect(run(FOLLOWING, { type: 'scroll', top: 600, max: 1000, moved: -400 }).following).toBe(false);
  // a scroll at a stale flight once paused stays paused (the reader happened to land there)
  expect(run(PAUSED, { type: 'pinned', top: 500 }, { type: 'scroll', top: 500, max: 1000, moved: -500 }).following).toBe(false);
  // back within END_PX of the end: following again
  expect(run(PAUSED, { type: 'scroll', top: 1000 - END_PX, max: 1000, moved: 300 }).following).toBe(true);
  expect(run(PAUSED, { type: 'scroll', top: 1000 - END_PX - 1, max: 1000, moved: 300 }).following).toBe(false);
  // Jump to latest, a follow-up
  expect(run(PAUSED, { type: 'end' }).following).toBe(true);
});

test('jump_to_latest_shows_only_while_paused_and_away_from_the_end', () => {
  const box = (scrollTop: number) => ({ scrollTop, scrollHeight: 2000, clientHeight: 800 });
  expect(awayFromEnd(PAUSED, box(400))).toBe(true);
  expect(awayFromEnd(PAUSED, box(1200 - END_PX))).toBe(false);
  // following: never, even a frame behind the end while a chunk lands
  expect(awayFromEnd(FOLLOWING, box(400))).toBe(false);
});
