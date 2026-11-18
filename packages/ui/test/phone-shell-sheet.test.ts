// QA round 6 phone #1 (major): on iOS the Shell in the side panel's sheet was an 8 px black bar. The sheet's body is a
// block, so the panel's `grow` gave it no height (Android's sheet already gave its panel the body's height). The panel
// fills the sheet on both looks; with the keyboard up the page knows (the key row's binding marks the body with
// `data-kb` and `--k-kb`), and the iOS sheet that holds the Shell goes to its large height and keeps the keyboard and the
// riding key row out of the terminal's way.
import { expect, test } from 'vitest';
import IOS from '../src/css/platform/ios.css?raw';
import ANDROID from '../src/css/platform/android.css?raw';
import { bindAccessory } from '../src/terminal/accessory.ts';

const css = (s: string): string => s.replace(/\/\*[^]*?\*\//g, '');
const rule = (s: string, sel: RegExp): string => new RegExp(`${sel.source}\\s*\\{([^}]*)\\}`).exec(css(s))?.at(-1) ?? '';

test('the_side_panel_fills_the_phone_sheet_on_ios_as_on_android', () => {
  const sel = /:root\[data-platform='(ios|android)'\] \[data-kit='sheet'\]\[data-part~='phone\.side\.sheet'\] \[data-part~='side\.panel'\]/;
  expect(rule(IOS, sel)).toMatch(/height:\s*100%/);
  expect(rule(ANDROID, sel)).toMatch(/height:\s*100%/);
});

test('with_the_keyboard_up_the_ios_shell_sheet_is_large_and_keeps_the_keyboard_and_key_row_clear', () => {
  const r = rule(IOS, /:root\[data-platform='ios'\] \[data-kb\] \[data-kit='sheet'\]\[data-part~='phone\.side\.sheet'\]/);
  expect(r).toMatch(/height:\s*calc\(100dvh - var\(--safe-top\) - var\(--sheet-top-gap\)\)/);
  expect(r).toMatch(/padding-bottom:\s*calc\(var\(--k-kb, var\(--space-0\)\) \+ var\(--tap-min\)/);
});

test('the_key_rows_binding_tells_the_page_while_the_keyboard_is_up', () => {
  const fns: Record<string, () => void> = {};
  const vv = { height: 844, offsetTop: 0, addEventListener: (t: string, fn: () => void) => void (fns[t] = fn), removeEventListener: (t: string) => void delete fns[t] };
  const el = document.createElement('div');
  const root = document.body;
  const stop = bindAccessory(el, { innerHeight: 844, visualViewport: vv });
  expect(root.hasAttribute('data-kb')).toBe(false);
  vv.height = 500;
  fns['resize']?.();
  expect(root.hasAttribute('data-kb')).toBe(true);
  expect(root.style.getPropertyValue('--k-kb')).toBe('344px');
  vv.height = 844;
  fns['resize']?.();
  expect(root.hasAttribute('data-kb')).toBe(false);
  vv.height = 500;
  fns['resize']?.();
  stop();
  expect(root.hasAttribute('data-kb')).toBe(false);
  expect(root.style.getPropertyValue('--k-kb')).toBe('');
});

// two terminals bound at once (the sheet's Shell and another kept open): the first to let go leaves the page's flag to
// the one still up
test('one_of_two_key_rows_letting_go_leaves_the_keyboard_flag_to_the_other', () => {
  const fns: (() => void)[] = [];
  const vv = { height: 500, offsetTop: 0, addEventListener: (_t: string, fn: () => void) => void fns.push(fn), removeEventListener: () => undefined };
  const a = bindAccessory(document.createElement('div'), { innerHeight: 844, visualViewport: vv });
  const b = bindAccessory(document.createElement('div'), { innerHeight: 844, visualViewport: vv });
  expect(document.body.hasAttribute('data-kb')).toBe(true);
  a();
  expect(document.body.hasAttribute('data-kb'), 'the other still has the keyboard').toBe(true);
  expect(document.body.style.getPropertyValue('--k-kb')).toBe('344px');
  b();
  expect(document.body.hasAttribute('data-kb')).toBe(false);
});
