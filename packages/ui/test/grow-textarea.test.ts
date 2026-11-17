// The launcher's prompt grows with its text up to v1's 340 px, then scrolls (owner, 2026-11-17: a long prompt
// scrolled inside a 64 px box on the phone's New task).
import { describe, expect, it } from 'vitest';
import { growTextarea } from '../src/core/autosize.ts';

const area = (content: number): HTMLTextAreaElement => {
  const el = document.createElement('textarea');
  Object.defineProperty(el, 'scrollHeight', { get: () => content });
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
});
