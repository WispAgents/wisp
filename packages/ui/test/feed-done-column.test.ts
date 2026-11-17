// QA round 1 #30: on a phone the row's time sat 10 px right of its status pill: the pill ends where the Mark done
// column and the grid's gap begin, the time only cleared the column. Both now end on one edge: the column says its
// width with its gap (--k-feed-done-x), and the time keeps clear of that.
import { expect, test } from 'vitest';
import { feedDoneColumn } from '../src/composites/feed-done.ts';

test('the_done_column_says_its_width_with_the_gap_before_it', () => {
  expect(feedDoneColumn(true)).toEqual({ '--k-feed-done': '36px', '--k-feed-done-pad': '48px', '--k-feed-done-x': '46px' });
  expect(feedDoneColumn(false)).toEqual({});
});
