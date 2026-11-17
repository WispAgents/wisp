import { expect, test } from 'vitest';
import { completionOf, ghostOf, suggestedFollowUps } from '../src/frame/follow-ghost.ts';

test('an_answers_next_block_is_what_tab_offers_in_its_order_one_line_each', () => {
  const blocks = [
    { type: 'hero', title: 'Done' },
    { type: 'next', items: ['Add a test for it', { label: 'Ship it', reply: 'Commit this and open a PR' }, { label: 'Two\nlines' }, 'Add a test for it'] },
  ];
  expect(suggestedFollowUps(blocks)).toEqual(['Add a test for it', 'Commit this and open a PR']);
  expect(suggestedFollowUps(undefined)).toEqual([]);
  expect(suggestedFollowUps([{ type: 'next' }])).toEqual([]);
});

test('the_ghost_is_the_rest_of_the_first_suggestion_that_starts_with_what_is_typed', () => {
  const s = ['Add a test for it', 'Commit this and open a PR'];
  expect(ghostOf('', s)).toBe('Add a test for it');
  expect(ghostOf('com', s)).toBe('mit this and open a PR'); // any case: the rest as the suggestion writes it
  expect(ghostOf('COMMIT THIS', s)).toBe(' and open a PR');
  expect(ghostOf('Commit this and open a PR', s)).toBe(''); // all written
  expect(ghostOf('Fix', s)).toBe('');
  expect(ghostOf('Add\n', s)).toBe(''); // a second line: nothing
  expect(completionOf('com', s)).toBe('Commit this and open a PR'); // Tab writes it as the answer did
});
