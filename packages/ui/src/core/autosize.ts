// Grows a textarea with its text up to a cap, then it scrolls. Shared by TextArea and Composer.
// Per key it reads layout once (perf-ui-r2 #11: two `scrollHeight` reads and a `getComputedStyle` a key made 5 style
// recalcs and 5 layouts per character): the line height and padding are read once per box, and text only added at the
// end (the usual key) can only grow the box, so it is measured where it stands, with no shrink-to-min first.

interface Fit {
  readonly line: number;
  readonly chrome: number;
  /** what the last fit measured: the value, the box's width, and the height it set */
  value?: string;
  width?: number;
  height?: string;
  rows?: string;
}

const fits = new WeakMap<HTMLTextAreaElement, Fit>();

function metrics(el: HTMLTextAreaElement): Fit {
  let f = fits.get(el);
  if (!f) {
    const cs = getComputedStyle(el);
    const line = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.5 || 20;
    const chrome = (parseFloat(cs.paddingTop) || 0) + (parseFloat(cs.paddingBottom) || 0) + (parseFloat(cs.borderTopWidth) || 0) + (parseFloat(cs.borderBottomWidth) || 0);
    f = { line, chrome };
    // only what a drawn box says is kept: detached or hidden, its computed style is empty (the fallbacks above)
    if (el.isConnected && cs.fontSize !== '') fits.set(el, f);
  }
  return f;
}

/** Fit `el` to its content, between `minRows` and `maxRows` lines. */
export function fitTextarea(el: HTMLTextAreaElement, minRows: number, maxRows: number): void {
  const f = metrics(el);
  const min = minRows * f.line + f.chrome;
  const max = maxRows * f.line + f.chrome;
  const value = el.value;
  const rows = `${minRows}:${maxRows}`;
  // only added to, in the same box as last time: its text can't need less room, so measure it as it stands
  const added = f.value !== undefined && value.startsWith(f.value) && f.rows === rows && el.style.height === f.height && el.clientWidth === f.width;
  let measured: number;
  if (added) {
    measured = el.scrollHeight;
    if (measured <= el.clientHeight) {
      f.value = value;
      return; // it still fits: nothing to write
    }
  } else {
    // Measure from the smallest height so it can shrink as well as grow.
    el.style.height = `${min}px`;
    measured = el.scrollHeight;
  }
  const height = `${Math.min(Math.max(measured, min), max)}px`;
  if (el.style.height !== height) el.style.height = height;
  const overflow = measured > max ? 'auto' : 'hidden';
  if (el.style.overflowY !== overflow) el.style.overflowY = overflow;
  Object.assign(f, { value, rows, height, width: el.clientWidth });
}

/** v1's autosize (web/js/features/attachments.js): grow with the text up to `max` px, then scroll. The launcher's
 *  prompt (v1 launcher.js: 340) - v2 had it fixed at its 64 px minimum, so any longer prompt scrolled inside. */
export function growTextarea(el: HTMLTextAreaElement, max: number): void {
  // empty and never grown: its stylesheet's height is right, and measuring it forced a style and layout pass on the first
  // screen (perf r1 #4: 59 ms at 4×)
  if (el.value === '' && el.style.height === '') return;
  el.style.height = 'auto';
  // one read: the height it then gets is this or the cap, so this alone says whether it scrolls (r2 #11)
  const h = el.scrollHeight;
  el.style.height = `${Math.min(max, h)}px`;
  el.style.overflowY = h > max ? 'auto' : 'hidden';
}
