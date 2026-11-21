// ADR-0065 step 13: a phone's task page steps to the next / previous task in the list's order (the head's swipe and
// its twins Next task / Previous task in the menu). Edge cases: the list's first and last task, a filter that changes
// the order while the page is open, a task deleted (or filtered out) meanwhile.
import { expect, test } from 'vitest';
import { placeIn, taskSteps } from '../src/frame/task-step.ts';

test('the_next_and_previous_task_are_its_neighbours_in_the_lists_order', () => {
  expect(taskSteps(['a', 'b', 'c'], 'b', undefined)).toEqual({ next: 'c', previous: 'a' });
});

test('the_first_task_has_no_previous_and_the_last_no_next_and_nothing_wraps', () => {
  expect(taskSteps(['a', 'b', 'c'], 'a', undefined)).toEqual({ next: 'b' });
  expect(taskSteps(['a', 'b', 'c'], 'c', undefined)).toEqual({ previous: 'b' });
  expect(taskSteps(['a'], 'a', undefined)).toEqual({});
  expect(taskSteps([], 'a', undefined)).toEqual({});
});

test('a_filter_that_changes_the_order_moves_the_neighbours_with_it', () => {
  const before = taskSteps(['a', 'b', 'c', 'd'], 'c', undefined);
  expect(before).toEqual({ next: 'd', previous: 'b' });
  // another project picked: the list now shows d, c, x
  expect(taskSteps(['d', 'c', 'x'], 'c', 2)).toEqual({ next: 'x', previous: 'd' });
});

test('a_task_that_left_the_list_meanwhile_keeps_its_place_between_its_old_neighbours', () => {
  // b was at 1 and was deleted on another device: c moved into its slot
  expect(taskSteps(['a', 'c', 'd'], 'b', 1)).toEqual({ next: 'c', previous: 'a' });
  // it was the last one
  expect(taskSteps(['a', 'c'], 'd', 2)).toEqual({ previous: 'c' });
  expect(taskSteps(['a', 'c'], 'd', 7)).toEqual({ previous: 'c' });
  // it was the first one
  expect(taskSteps(['c', 'd'], 'a', 0)).toEqual({ next: 'c' });
  // never in the list (opened from search, a filter hides it): no neighbours to step to
  expect(taskSteps(['a', 'c'], 'z', undefined)).toEqual({});
});

test('a_neighbour_deleted_meanwhile_is_skipped', () => {
  // b's next was c; c is gone, so d is next
  expect(taskSteps(['a', 'b', 'd'], 'b', 1)).toEqual({ next: 'd', previous: 'a' });
});

test('the_place_is_where_it_sits_now_else_where_it_last_sat', () => {
  expect(placeIn(['a', 'b'], 'b', undefined)).toBe(1);
  expect(placeIn(['a'], 'b', 1)).toBe(1);
  expect(placeIn(['a'], 'b', undefined)).toBeUndefined();
});
