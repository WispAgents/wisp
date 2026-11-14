/// <reference types="vite/client" />
// The phone apps' sheets (src/css/platform/*.css, ADR-0065 §5) follow the kit's CSS rules, with one
// difference: they restate the layer order (they load on their own) and every rule is scoped to its look.
import { expect, test } from 'vitest';
import { PX_SCALE } from '@wisp/tokens';

const files = import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true });
const sheets = Object.entries(files).map(([path, raw]) => ({ look: path.replace(/^.*\/|\.css$/g, ''), css: raw.replace(/\/\*[^]*?\*\//g, '') }));
// M3 window size classes (compact < 600 dp, expanded ≥ 840 dp) and the browser's split (900): the only px in a query
const PLATFORM_BREAKPOINTS = new Set([600, 840, 900]);

/** Every rule as [selector, body, the at-rules it sits in]. */
function rules(css: string): Array<[string, string, string]> {
  const out: Array<[string, string, string]> = [];
  const stack: string[] = [];
  let head = '';
  for (let i = 0; i < css.length; i++) {
    const c = css[i];
    if (c === '{') {
      const sel = head.trim();
      head = '';
      if (sel.startsWith('@')) {
        stack.push(sel);
        continue;
      }
      const end = css.indexOf('}', i);
      out.push([sel, css.slice(i + 1, end), stack.join(' ')]);
      i = end;
    } else if (c === '}') {
      stack.pop();
      head = '';
    } else if (c === ';' && stack.length === 0) head = '';
    else head += c;
  }
  return out;
}

test('there_is_a_sheet_per_phone_look_with_the_frame_and_navigation_variants', () => {
  expect(sheets.map((s) => s.look).sort()).toEqual(['android', 'ios']);
  for (const s of sheets) for (const kit of ['frame', 'frame-top', 'nav-bar', 'nav-item', 'app-bar', 'app-bar-large']) expect(s.css, `${s.look} ${kit}`).toContain(`[data-kit='${kit}']`);
  expect(sheets.find((s) => s.look === 'android')?.css).toContain("[data-kit='fab']");
});

test('every_platform_rule_sits_in_layer_kit_under_its_own_look', () => {
  for (const s of sheets) {
    const all = rules(s.css);
    expect(all.length, s.look).toBeGreaterThan(20);
    for (const [sel, , at] of all) {
      expect(at, `${s.look}: ${sel}`).toMatch(/^@layer kit/);
      for (const one of sel.split(',')) expect(one.trim(), s.look).toMatch(new RegExp(`^:root\\[data-platform='${s.look}'\\] `));
    }
  }
});

test('platform_px_come_from_tokens_and_queries_only_from_the_size_classes', () => {
  const scale = new Set(PX_SCALE);
  const bad: string[] = [];
  for (const s of sheets) {
    const body = s.css.replace(/@import[^;]*;/g, '');
    for (const m of body.matchAll(/(@media[^{]*)\{|(-?\d*\.?\d+)px/g)) {
      if (m[1]) for (const q of m[1].matchAll(/(\d+)px/g)) if (!PLATFORM_BREAKPOINTS.has(Number(q[1]))) bad.push(`${s.look}: ${q[0]} in a query`);
      if (m[2] && !scale.has(Math.abs(Number(m[2])))) bad.push(`${s.look}: ${m[2]}px`);
    }
    // a raw px at all is a missed token: these sheets use the platform's own
    expect(body.replace(/@media[^{]*\{/g, ''), s.look).not.toMatch(/\d px|\dpx/);
  }
  expect(bad).toEqual([]);
});

test('platform_sheets_keep_the_kit_rules_no_has_no_is_no_colours_no_wrapping', () => {
  for (const s of sheets) {
    expect(s.css, s.look).not.toMatch(/:has\(|:is\(/);
    expect(s.css, s.look).not.toMatch(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|:\s*(white|black)\b/);
    expect(s.css, s.look).not.toMatch(/flex-wrap:\s*wrap|white-space:\s*normal/);
    expect(s.css, s.look).not.toMatch(/(^|[\s,>+~(])\.[a-zA-Z_-][\w-]*\s*[{,:[]/m);
    expect(s.css, s.look).not.toMatch(/[+~]\s*\*/);
  }
});

test('nav_labels_and_bar_titles_never_wrap', () => {
  // the nav's label and the title button's text are [data-text='control'] (content.css: nowrap, ellipsis)
  for (const s of sheets) {
    const titles = rules(s.css).filter(([sel]) => /\[data-kit='app-bar-title'\]$/.test(sel));
    expect(titles.some(([sel, body]) => sel.endsWith(`:root[data-platform='${s.look}'] [data-kit='app-bar-title']`) && /white-space: nowrap/.test(body) && /text-overflow: ellipsis/.test(body)), s.look).toBe(true);
    for (const [sel, body] of titles) expect(body, sel).not.toMatch(/white-space:\s*(normal|pre-wrap)|line-clamp/);
  }
});
