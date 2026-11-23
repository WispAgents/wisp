// Each task keeps its place (src/frame/place.ts, v1 split.js keepPlace/backToPlace): by the turn at the top, or the end.
import { expect, test } from 'vitest';
import { keep, placeOf, scrollFor, MAX_PLACES } from '../src/frame/place.ts';

const turns = [
  { id: 't1', top: 0 },
  { id: 't2', top: 800 },
  { id: 't3', top: 1600 },
];

test('a task reopens where it was read, held by its turn, so a new turn above it shifts nothing_v1_keep_place', () => {
  const p = placeOf({ scrollTop: 950, clientHeight: 600, scrollHeight: 3000 }, turns);
  expect(p).toEqual({ end: false, turn: 't2', dy: 150, top: 950 });
  // later, older turns folded away (t2 now at 300): the same line of t2 is at the top
  expect(
    scrollFor(p!, { clientHeight: 600, scrollHeight: 2500 }, [
      { id: 't2', top: 300 },
      { id: 't3', top: 1100 },
    ]),
  ).toBe(450);
  // at the end: back to the end, new turns and all
  const end = placeOf({ scrollTop: 2400, clientHeight: 600, scrollHeight: 3000 }, turns);
  expect(end?.end).toBe(true);
  expect(scrollFor(end!, { clientHeight: 600, scrollHeight: 3600 }, turns)).toBe(3000);
  // at the top: nothing kept; its turn gone: the raw offset
  expect(placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 3000 }, turns)).toBeUndefined();
  expect(scrollFor({ end: false, turn: 'gone', dy: 10, top: 700 }, { clientHeight: 600, scrollHeight: 3000 }, turns)).toBe(700);
});

test('chat_mode_saves_following_as_end_and_preserves_an_explicit_top_scroll_on_a_long_task', () => {
  // following near the bottom (even > 4 px away while streaming settles) always saves end: true
  expect(placeOf({ scrollTop: 2370, clientHeight: 600, scrollHeight: 3000 }, turns, { following: true, chat: true })?.end).toBe(true);
  // scrolled all the way up to turn 1 in Chat mode: preserved explicitly so reopening restores turn 1 instead of jumping to end
  expect(placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 3000 }, turns, { following: false, chat: true })).toEqual({
    end: false,
    turn: 't1',
    dy: 0,
    top: 0,
  });
  // a short task that fits on screen still keeps nothing when transcript view is off
  expect(placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 600 }, turns, { following: true, chat: true })).toBeUndefined();
});

test('transcript_view_is_off_by_default_and_persisted_per_task_alongside_scroll_position', () => {
  // short Chat task at scrollTop 0 with Transcript turned on keeps transcript: true and end: true
  const shortChatTx = placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 400 }, turns, { following: true, chat: true, transcript: true });
  expect(shortChatTx).toEqual({ end: true, dy: 0, top: 0, transcript: true });

  // short Cards task at scrollTop 0 with Transcript turned on keeps transcript: true and end: true (fits on screen)
  const shortCardsTx = placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 400 }, turns, { following: false, chat: false, transcript: true });
  expect(shortCardsTx).toEqual({ end: true, dy: 0, top: 0, transcript: true });

  // long Cards task at scrollTop 0 with Transcript turned on keeps transcript: true and end: false
  const longCardsTx = placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 3000 }, turns, { following: false, chat: false, transcript: true });
  expect(longCardsTx).toEqual({ end: false, dy: 0, top: 0, transcript: true });
  expect(scrollFor(longCardsTx!, { clientHeight: 600, scrollHeight: 3000 }, turns)).toBe(0);

  // mid-page scroll in Chat mode with Transcript turned on preserves both turn offset and transcript: true
  const midChatTx = placeOf({ scrollTop: 950, clientHeight: 600, scrollHeight: 3000 }, turns, { following: false, chat: true, transcript: true });
  expect(midChatTx).toEqual({ end: false, turn: 't2', dy: 150, top: 950, transcript: true });
  expect(scrollFor(midChatTx!, { clientHeight: 600, scrollHeight: 3000 }, turns)).toBe(950);

  // scrolled to top of long Chat task with Transcript turned on preserves turn 1 and transcript: true
  const topChatTx = placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 3000 }, turns, { following: false, chat: true, transcript: true });
  expect(topChatTx).toEqual({ end: false, turn: 't1', dy: 0, top: 0, transcript: true });

  // turning Transcript back off at default position returns undefined so keep() drops the entry
  const offAtDefault = placeOf({ scrollTop: 0, clientHeight: 600, scrollHeight: 400 }, turns, { following: true, chat: true, transcript: false });
  expect(offAtDefault).toBeUndefined();
});

test('the most recent hundred tasks keep theirs, the oldest go first_v1_max_places', () => {
  let all = {};
  for (let i = 0; i < MAX_PLACES + 5; i++) all = keep(all, `s${i}`, { end: false, dy: 0, top: i + 1 });
  expect(Object.keys(all)).toHaveLength(MAX_PLACES);
  expect(Object.keys(all)[0]).toBe('s5');
  expect(Object.keys(keep(all, 's7', undefined))).not.toContain('s7');
  expect(Object.keys(keep(all, 's5', { end: true, dy: 0, top: 1 })).at(-1)).toBe('s5'); // touched: newest
});
