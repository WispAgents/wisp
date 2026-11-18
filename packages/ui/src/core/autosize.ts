// Grows a textarea with its text up to a cap, then it scrolls. Shared by TextArea and Composer.

/** Fit `el` to its content, between `minRows` and `maxRows` lines. */
export function fitTextarea(el: HTMLTextAreaElement, minRows: number, maxRows: number): void {
  const cs = getComputedStyle(el);
  const line = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.5 || 20;
  const chrome = (parseFloat(cs.paddingTop) || 0) + (parseFloat(cs.paddingBottom) || 0) + (parseFloat(cs.borderTopWidth) || 0) + (parseFloat(cs.borderBottomWidth) || 0);
  const min = minRows * line + chrome;
  const max = maxRows * line + chrome;
  // Measure from the smallest height so it can shrink as well as grow.
  el.style.height = `${min}px`;
  const want = Math.min(Math.max(el.scrollHeight, min), max);
  el.style.height = `${want}px`;
  el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden';
}

/** v1's autosize (web/js/features/attachments.js): grow with the text up to `max` px, then scroll. The launcher's
 *  prompt (v1 launcher.js: 340) - v2 had it fixed at its 64 px minimum, so any longer prompt scrolled inside. */
export function growTextarea(el: HTMLTextAreaElement, max: number): void {
  // empty and never grown: its stylesheet's height is right, and measuring it forced a style and layout pass on the first
  // screen (perf r1 #4: 59 ms at 4×)
  if (el.value === '' && el.style.height === '') return;
  el.style.height = 'auto';
  el.style.height = `${Math.min(max, el.scrollHeight)}px`;
  el.style.overflowY = el.scrollHeight > max ? 'auto' : 'hidden';
}
