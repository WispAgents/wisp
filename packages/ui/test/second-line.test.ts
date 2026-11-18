// QA round 3 desktop #6: a row with no summary and no pill kept an empty second line between its title and its run
// line. The feed row reads its own second line: empty (the sub has nothing, the pills' cell only the time, or a
// plugin's pill that draws nothing), the row draws no blank line.
import { expect, test } from 'vitest';
import { secondLineEmpty } from '../src/composites/second-line.ts';

const el = (html: string): HTMLElement => {
  const d = document.createElement('div');
  d.innerHTML = html;
  return d;
};

test('the_second_line_is_empty_with_no_words_and_only_the_time', () => {
  expect(secondLineEmpty(el(''), el('<time data-kit="time">5h</time>'))).toBe(true);
  // a plugin's slot that drew nothing
  expect(secondLineEmpty(el(''), el('<div data-kit="extension"></div><time data-kit="time">5h</time>'))).toBe(true);
  expect(secondLineEmpty(el('You stopped this run'), el('<time data-kit="time">5h</time>'))).toBe(false);
  expect(secondLineEmpty(el(''), el('<span data-kit="it-pill">Report</span><time data-kit="time">5h</time>'))).toBe(false);
  // an icon-only pill counts
  expect(secondLineEmpty(el(''), el('<span data-kit="pr-pill"><svg></svg></span><time data-kit="time">5h</time>'))).toBe(false);
  expect(secondLineEmpty(el('<svg></svg>'), el(''))).toBe(false);
});
