// A picture growing in a task (app/media-shift.ts, v1 tail.js mediaShift #776).
import { expect, test } from 'vitest';
import { atTail, dockShift, mediaShift, turnsShift } from '../src/app/media-shift.ts';

test('a_picture_loading_in_a_task_keeps_the_view_where_the_reader_had_it', () => {
  const grew = [{ top: 40, from: 0, to: 300 }];
  // at the end: back to the end
  expect(mediaShift({ tail: true, anchors: false, inset: 100, grew })).toBe('end');
  // Chrome anchors the scroll itself: nothing to do
  expect(mediaShift({ tail: false, anchors: true, inset: 100, grew })).toBe(0);
  // WebKit: one wholly above the reading line pushed the text down by what it grew; one below moves nothing
  expect(mediaShift({ tail: false, anchors: false, inset: 100, grew })).toBe(300);
  expect(mediaShift({ tail: false, anchors: false, inset: 100, grew: [{ top: 500, from: 0, to: 300 }] })).toBe(0);
  // two above: the lower one's top already counts the upper one's growth, and both are undone
  expect(mediaShift({ tail: false, anchors: false, inset: 400, grew: [{ top: 330, from: 0, to: 120 }, { top: 10, from: 0, to: 300 }] })).toBe(420);
  expect(atTail({ scrollHeight: 1000, scrollTop: 599, clientHeight: 400 })).toBe(true);
  expect(atTail({ scrollHeight: 1000, scrollTop: 500, clientHeight: 400 })).toBe(false);
});

test('the_reply_box_growing_at_the_tail_keeps_the_last_block_above_it', () => {
  // at the end (2891 + 844 = 3735 before), the box grew by 30 (focus showed its bar): back to the end
  expect(dockShift({ scrollHeight: 3765, scrollTop: 2891, clientHeight: 844, dock: 30, view: 0 })).toBe('end');
  // the keyboard took 380 px off the view at the end: back to the end
  expect(dockShift({ scrollHeight: 3735, scrollTop: 2891, clientHeight: 464, dock: 0, view: -380 })).toBe('end');
  // both at once (focus on a phone)
  expect(dockShift({ scrollHeight: 3765, scrollTop: 2891, clientHeight: 464, dock: 30, view: -380 })).toBe('end');
  // reading further up: the sticky box covers what's under it and moves nothing
  expect(dockShift({ scrollHeight: 3765, scrollTop: 1200, clientHeight: 844, dock: 30, view: 0 })).toBe(0);
  // a short task that fit: the box growing makes it overflow, and the end stays in view
  expect(dockShift({ scrollHeight: 874, scrollTop: 0, clientHeight: 844, dock: 30, view: 0 })).toBe('end');
  // a shrink or a sub-pixel change needs nothing (the browser clamps)
  expect(dockShift({ scrollHeight: 3705, scrollTop: 2861, clientHeight: 844, dock: -30, view: 0 })).toBe(0);
  expect(dockShift({ scrollHeight: 3735, scrollTop: 2891, clientHeight: 844, dock: 0.4, view: 0 })).toBe(0);
});

// F-page-44 (v1 turns.js renderBody: only the tail moves the view): a turn landing while you read keeps you on it.
test('a_turn_that_lands_while_you_read_keeps_the_view_put_and_at_the_end_you_go_to_it', async () => {
  // reading further up: Chrome's anchoring holds it; WebKit undoes what moved the turn under the reading line
  expect(turnsShift({ tail: false, anchors: true, landed: true, held: { from: 40, to: 240 } })).toBe(0);
  expect(turnsShift({ tail: false, anchors: false, landed: true, held: { from: 40, to: 240 } })).toBe(200);
  expect(turnsShift({ tail: false, anchors: false, landed: false, held: { from: 40, to: 40 } })).toBe(0);
  expect(turnsShift({ tail: false, anchors: false, landed: false })).toBe(0);
  // at the end: a landed turn is shown from its start; the running turn's steps keep the end in view
  expect(turnsShift({ tail: true, anchors: true, landed: true })).toBe('land');
  expect(turnsShift({ tail: true, anchors: false, landed: false })).toBe('end');
});

test('chat_mode_never_goes_back_to_a_landed_turns_start_it_follows_the_end_or_pages_down_when_more_came_than_fits', () => {
  // following the end: a turn finishing keeps the end in view (Cards shows it from its start)
  expect(turnsShift({ tail: true, anchors: true, landed: true, chat: true, grew: 200, room: 600 })).toBe('end');
  expect(turnsShift({ tail: true, anchors: true, landed: true, chat: false, grew: 200, room: 600 })).toBe('land');
  // an answer taller than the view lands at once: one view down, so it reads from where the reader was
  expect(turnsShift({ tail: true, anchors: true, landed: true, chat: true, grew: 1056, room: 600 })).toBe('page');
  // streaming (nothing landed) follows the end however much came
  expect(turnsShift({ tail: true, anchors: true, landed: false, chat: true, grew: 1056, room: 600 })).toBe('end');
  // a reader who scrolled away is never moved, in either mode
  expect(turnsShift({ tail: false, anchors: true, landed: true, chat: true, grew: 1056, room: 600 })).toBe(0);
});
