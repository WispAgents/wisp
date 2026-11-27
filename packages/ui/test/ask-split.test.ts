// Owner 2026-11-27: an agent's question carries its whole explanation; the card shows the call as its headline and
// the rest as context (v1 .q-text / .q-context).
import { expect, test } from 'vitest';
import { splitQuestion } from '../src/blocks/ask-split.ts';

test('a_question_splits_at_its_first_paragraph_or_list_ask_split', () => {
  expect(splitQuestion('Ship it?')).toEqual({ headline: 'Ship it?', context: '' });
  expect(splitQuestion('Ship it?\n\nCI is green.')).toEqual({ headline: 'Ship it?', context: 'CI is green.' });
  expect(splitQuestion('Pick one:\n- a\n- b')).toEqual({ headline: 'Pick one:', context: '- a\n- b' });
  // one line wrapped by hand stays the headline
  expect(splitQuestion('Merge #812\nand tag it?')).toEqual({ headline: 'Merge #812\nand tag it?', context: '' });
  // one long paragraph: the first question is the call
  const long = `Should the new Push button open a PR? ${'It writes the history files and never commits them. '.repeat(4)}`;
  expect(splitQuestion(long).headline).toBe('Should the new Push button open a PR?');
  // a short one keeps a question mark in the middle whole
  expect(splitQuestion('Why? Because CI is red.').context).toBe('');
});
