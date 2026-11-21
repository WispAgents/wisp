// The launcher's prompt grows with its text up to v1's 340 px, then scrolls (owner, 2026-11-17: a long prompt
// scrolled inside a 64 px box on the phone's New task).
import { describe, expect, it } from 'vitest';
import { growTextarea } from '../src/core/autosize.ts';

const area = (content: number | (() => number), value = 'a prompt'): HTMLTextAreaElement => {
  const el = document.createElement('textarea');
  el.value = value;
  Object.defineProperty(el, 'scrollHeight', { get: () => (typeof content === 'number' ? content : content()) });
  return el;
};

describe('growTextarea', () => {
  it('a_prompt_grows_to_its_text_with_no_scrollbar', () => {
    const el = area(180);
    growTextarea(el, 340);
    expect(el.style.height).toBe('180px');
    expect(el.style.overflowY).toBe('hidden');
  });
  it('a_prompt_past_the_cap_stops_at_it_and_scrolls', () => {
    const el = area(667);
    growTextarea(el, 340);
    expect(el.style.height).toBe('340px');
    expect(el.style.overflowY).toBe('auto');
  });
  // perf r1 #4: the empty prompt on the first screen was measured (a forced style and layout pass) for nothing
  it('an_empty_prompt_never_grown_is_left_to_its_stylesheet_and_one_emptied_shrinks_back', () => {
    let read = 0;
    const el = area(() => (read++, 0), '');
    growTextarea(el, 340);
    expect(read, 'not measured').toBe(0);
    expect(el.style.height).toBe('');
    let h = 180;
    const grown = area(() => h);
    growTextarea(grown, 340);
    grown.value = '';
    h = 60;
    growTextarea(grown, 340);
    expect(grown.style.height, 'emptied: back to its text').toBe('60px');
  });
});
