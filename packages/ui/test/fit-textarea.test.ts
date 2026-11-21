// perf-ui-r2 #11: each key in the reply box or New task's prompt read `scrollHeight` twice and called
// `getComputedStyle` (5 style recalcs and 5 layouts a key). Now one layout read a key, the box's metrics once, and the
// usual key (text added at the end) neither shrinks the box to measure it nor writes when nothing changed.
import { afterEach, expect, test, vi } from 'vitest';
import { fitTextarea } from '../src/core/autosize.ts';

afterEach(() => vi.restoreAllMocks());

/** A textarea whose content is `lines()` lines of 20 px, 300 px wide, counting its layout reads. */
function area(lines: () => number) {
  const el = document.createElement('textarea');
  document.body.append(el);
  let reads = 0;
  const box = () => parseFloat(el.style.height) || 40;
  Object.defineProperty(el, 'scrollHeight', { get: () => (reads++, Math.max(lines() * 20, box())) });
  Object.defineProperty(el, 'clientHeight', { get: () => box() });
  Object.defineProperty(el, 'clientWidth', { get: () => 300 });
  return { el, reads: () => reads };
}

test('typing_reads_layout_once_a_key_and_the_computed_style_once_per_box', () => {
  const cs = vi.spyOn(window, 'getComputedStyle').mockReturnValue({ lineHeight: '20px', fontSize: '14px', paddingTop: '0', paddingBottom: '0', borderTopWidth: '0', borderBottomWidth: '0' } as CSSStyleDeclaration);
  let n = 1;
  const { el, reads } = area(() => n);
  const heights: string[] = [];
  const set = vi.spyOn(el.style, 'setProperty');
  for (const ch of 'fix the flaky reconnect test') {
    el.value += ch;
    if (ch === ' ') n++; // each word wraps to a new line here
    fitTextarea(el, 1, 8);
    heights.push(el.style.height);
  }
  expect(cs).toHaveBeenCalledTimes(1);
  expect(reads(), 'one scrollHeight read a key').toBe(28);
  expect(heights.at(-1)).toBe('100px');
  expect(el.style.overflowY).toBe('hidden');
  // the first fit sets its height and overflow; after that a write only when a line is added (4 spaces, 4 lines)
  expect(set).toHaveBeenCalledTimes(2 + 4);
});

test('the_box_still_shrinks_when_text_goes_and_scrolls_past_its_cap', () => {
  vi.spyOn(window, 'getComputedStyle').mockReturnValue({ lineHeight: '20px', fontSize: '14px', paddingTop: '0', paddingBottom: '0', borderTopWidth: '0', borderBottomWidth: '0' } as CSSStyleDeclaration);
  let n = 12;
  const { el } = area(() => n);
  el.value = 'a long prompt';
  fitTextarea(el, 1, 8);
  expect([el.style.height, el.style.overflowY]).toEqual(['160px', 'auto']);
  el.value = 'a';
  n = 1;
  fitTextarea(el, 1, 8);
  expect([el.style.height, el.style.overflowY], 'deleted: back to one line').toEqual(['20px', 'hidden']);
  el.value = 'ab';
  fitTextarea(el, 1, 8);
  expect(el.style.height, 'a key that fits writes nothing new').toBe('20px');
});
