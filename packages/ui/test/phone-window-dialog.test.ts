// QA audit D #5: on Android, My Stuff's reader opened as a tinted M3 basic dialog (28 dp corners, 24 dp padding, short of
// the bottom) with the reader's own header inset in it: two surfaces. The M3 basic dialog is for a question; a window
// (`size: 'window'`: the reader, an app's window) is the whole screen on a phone, as on iOS and in a phone's browser.
import { expect, test } from 'vitest';
import ANDROID from '../src/css/platform/android.css?raw';

const css = (s: string): string => s.replace(/\/\*[^]*?\*\//g, '');
const rules = (s: string): { sel: string; body: string }[] => [...css(s).matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ sel: (m[1] ?? '').trim(), body: m[2] ?? '' }));

test('a_window_dialog_is_never_drawn_as_the_android_basic_dialog_v1_reader_full_screen', () => {
  const basic = rules(ANDROID).filter((r) => /\[data-kit='dialog(-positioner)?'\]\s*$/.test(r.sel.replace(/:not\([^)]*\)/g, '')) && /padding|border-radius|background|max-height/.test(r.body));
  expect(basic.length, 'the basic dialog rules').toBeGreaterThan(0);
  for (const r of basic) expect(r.sel, 'skips the window size').toContain(":not([data-size='window'])");
});
