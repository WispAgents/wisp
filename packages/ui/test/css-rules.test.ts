/// <reference types="vite/client" />
// The kit's CSS rules (plan §5), checked on every file in src/css: one layer order, every rule in
// a layer, px only from the token scale, no :has(), no :is(), no hard-coded colours.
import { expect, test } from 'vitest';
import { BREAKPOINT_PX, PX_SCALE } from '@wisp/tokens';
// The px and layout rules are `npm run check`'s own (@wisp/tokens/css-rules): one implementation, so a sheet
// fails both gates or neither (round 244: the check passed a raw 600px query and a data-part "…wrap" flex-wrap).
import { kitCssProblems, kitSheetProblems, type KitSheetRule } from '@wisp/tokens/css-rules';

const files = import.meta.glob<string>('../src/css/*.css', { query: '?raw', import: 'default', eager: true });
const sheets = Object.entries(files).map(([path, raw]) => ({ name: path.replace('../src/css/', ''), raw, css: raw.replace(/\/\*[^]*?\*\//g, '') }));
const SETS = { scale: new Set(PX_SCALE), breakpoints: new Set(BREAKPOINT_PX) };
const all = sheets.map((s) => ({ name: s.name, problems: kitSheetProblems(s.name, s.raw, SETS) }));
const problems = (kind: KitSheetRule): string[] => all.flatMap((s) => s.problems.filter((p) => p.kind === kind).map((p) => `${s.name}:${p.line} ${p.message}`));

test('there are stylesheets to check', () => {
  expect(sheets.length).toBeGreaterThan(5);
});

test('the layer order is declared once, in index.css, with plugins last', () => {
  expect(sheets.some((s) => s.name === 'index.css')).toBe(true);
  expect(problems('layer-order')).toEqual([]);
});

test('every other sheet is one @layer kit or @layer reset block', () => {
  expect(problems('layer-block')).toEqual([]);
});

test('px values come from the token scale (breakpoints only in @media / @container)', () => {
  expect(problems('px')).toEqual([]);
});

test('no :has() and no :is() (they make every style recalc walk large boxes)', () => {
  expect(problems('has-is')).toEqual([]);
});

test('no_sibling_combinator_lands_on_a_universal_selector', () => {
  // F-phone-8: `X > * + *` (or `X + *`) restyles every row of a list when one is added (rule in @wisp/tokens/css-rules)
  expect(problems('sibling-universal')).toEqual([]);
});

test('no hard-coded colours: every colour is a token', () => {
  expect(problems('colour')).toEqual([]);
});

test('the kit styles by data-kit attributes, never by class names', () => {
  expect(problems('class')).toEqual([]);
});

test('nothing wraps a row but Wrap: flex-wrap: wrap appears only on a Wrap, control text never wraps (design/layout.md §5, §6)', () => {
  expect(problems('layout')).toEqual([]);
  // the Wrap layout itself is still where wrapping lives
  const layout = sheets.find((x) => x.name === 'layout.css')?.css ?? '';
  expect(layout).toMatch(/\[data-kit='wrap'\], \[data-layout='wrap'\] \{[^}]*flex-wrap: wrap;/);
});

test('css_rules_the_shared_check_refuses_a_raw_px_query_and_a_flex_wrap_outside_wrap', () => {
  // the same function as `npm run check`: a 600px query (on the scale, not a breakpoint) and a data-part named …wrap
  const sets = { scale: new Set([0, 8, 600]), breakpoints: new Set([720]) };
  const bad = "@layer kit {\n  @container launcher (max-width: 600px) { [data-kit='a'] { gap: 8px; } }\n  [data-kit='seg'][data-part='x.account-wrap'] { flex-wrap: wrap; }\n  [data-kit='seg'][data-layout='wrap'] { gap: 8px; }\n  [data-kit='seg']:not([data-layout='wrap']) { flex-wrap: wrap; }\n}";
  expect(kitCssProblems(bad, sets).map((p) => [p.line, p.kind])).toEqual([
    [2, 'px'],
    [3, 'layout'],
    [5, 'layout'],
  ]);
});

test('control text never gets white-space: normal', () => {
  expect(problems('white-space-normal')).toEqual([]);
  const layout = sheets.find((x) => x.name === 'content.css')?.css ?? '';
  expect(layout).toMatch(/\[data-text='control'\]\s*\{[^}]*white-space: nowrap;[^}]*text-overflow: ellipsis;/);
});

test('a bar has a token height and never wraps', () => {
  const css = sheets.find((x) => x.name === 'composites.css')?.css ?? '';
  const rule = /\[data-kit='toolbar'\]\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';
  expect(rule).toMatch(/height: var\(--bar-h\)/);
  expect(rule).toMatch(/flex-wrap: nowrap/);
});

test('a_pressed_button_never_jumps_none_is_centred_by_transform_v1_782', () => {
  // the press is a transform (scale): a button centred by translate(-50%) jumps by half its width under the finger
  expect(problems('pressed-translate')).toEqual([]);
});

test('the_task_heads_title_is_one_line_at_every_width_v1_778', () => {
  // a subagent's title beside its lead's is one line too on the phone: no sheet lets the head's title wrap
  const rules = sheets.flatMap((s) => [...s.css.matchAll(/([^{}]+)\{([^}]*)\}/g)].filter((m) => (m[1] ?? '').includes("[data-kit='sh-title']")).map((m) => ({ file: s.name, body: m[2] ?? '' })));
  expect(rules.length).toBeGreaterThan(0);
  expect(problems('head-title-wraps')).toEqual([]);
  expect(rules.some((r) => /white-space: nowrap/.test(r.body) && /text-overflow: ellipsis/.test(r.body))).toBe(true);
});

/** Every rule of a sheet as [selector, body, the @media conditions it sits in]. */
function rulesIn(css: string): Array<[string, string, string]> {
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
      out.push([sel, css.slice(i + 1, end), stack.filter((x) => x.startsWith('@media')).join(' ')]);
      i = end;
    } else if (c === '}') {
      stack.pop();
      head = '';
    } else head += c;
  }
  return out;
}

test('on_touch_mark_done_keeps_its_own_column_after_the_time_and_save_for_later_leaves_v1_has_quick', () => {
  // seen at 390 (2026-11-14): on a touch screen Save for later and Mark done sat over the title, a letter peeking
  // between them. v1 parity (split.css #feed.has-quick): touch (hover: none) hides Save for later only; Mark done sits
  // in a 36px column of its own after the time, which exists only while the list sets --k-feed-done (no :has()).
  const rules = rulesIn(sheets.find((s) => s.name === 'composites.css')?.css ?? '');
  const row = "[data-kit='list-row'][data-layout='feed']";
  const quick = `${row} > [data-kit='icon-button']`;
  const touch = rules.filter(([, , media]) => /\(hover:\s*none\)/.test(media));
  const at = (sel: string): string => touch.find(([s]) => s === sel)?.[1] ?? '';
  expect(at(`${quick}:not([data-tone='good'])`)).toMatch(/display:\s*none/);
  const done = at(`${row}[data-quick] > [data-kit='icon-button'][data-tone='good']`);
  expect(done).toMatch(/right:\s*8px/);
  expect(done).toMatch(/width:\s*36px/);
  expect(done).toMatch(/opacity:\s*1/);
  // the column, the title's and the time's room for it come only from the list's var: unset, v1's three columns
  expect(at(row)).toMatch(/grid-template-columns:\s*10px minmax\(0, 1fr\) auto var\(--k-feed-done,\)/);
  expect(at(`${row} > [data-kit='list-row-title']`)).toMatch(/padding-right:\s*calc\(36px \+ var\(--k-feed-done-pad, 0px\)\)/);
  expect(touch.some(([sel, body]) => sel.includes("[data-kit='time']") && /right:\s*calc\(12px \+ var\(--k-feed-done, 0px\)\)/.test(body))).toBe(true);
  // every rule that pads the feed title for the floating quick buttons sits under (hover: hover)
  const reserve = rules.filter(([sel, body]) => sel.includes('[data-quick=') && sel.includes("[data-kit='list-row-title']") && /padding-right/.test(body));
  expect(reserve.length).toBe(6);
  for (const [sel, , media] of reserve) expect(media, sel).toMatch(/\(hover:\s*hover\)/);
});

const platformFiles = import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true });
/** Every rule of the kit's sheets and the phone looks' (platform/*.css) as [file, selector, body]. */
const everyRule = (): Array<[string, string, string]> =>
  [...sheets, ...Object.entries(platformFiles).map(([path, raw]) => ({ name: path.replace('../src/css/', ''), css: raw.replace(/\/\*[^]*?\*\//g, '') }))].flatMap((s) => rulesIn(s.css).map(([sel, body]): [string, string, string] => [s.name, sel, body]));

test('a_delegates_time_and_pill_stay_inside_the_leads_card_on_a_phone_v1_799', () => {
  // v1 #799: a long @name and the machine's chip pushed the time off the card at 320 px. v2's team rows (crew TeamPane,
  // ListRow: face, @name, what it's doing, state pill, time) are a grid whose title column is minmax(0, 1fr): the
  // name and line give way (one line, an ellipsis) and the pills and time keep their width, in the browser and in
  // both phone looks. Seen at 320/390 in target/r295/.
  const rules = everyRule();
  const grids = rules.filter(([, sel, body]) => /\[data-kit='list-row'\]/.test(sel) && /grid-template-columns:/.test(body));
  expect(grids.length).toBeGreaterThan(0);
  for (const [file, sel, body] of grids) expect(`${file} ${sel}: ${/grid-template-columns:\s*[^;]*minmax\(0, 1fr\)/.test(body)}`).toBe(`${file} ${sel}: true`);
  const own = (kit: string): string[] => rules.filter(([, sel]) => sel.split(',').some((x) => x.trim() === `[data-kit='${kit}']`)).map(([, , b]) => b);
  for (const kit of ['list-row-title', 'list-row-sub']) expect(own(kit).some((b) => /overflow: hidden/.test(b) && /text-overflow: ellipsis/.test(b) && /white-space: nowrap/.test(b))).toBe(true);
  expect(own('list-row-pills').some((b) => /flex-wrap: nowrap/.test(b))).toBe(true);
  // nothing lets the title or pills column grow past the card
  expect(rules.filter(([, sel, body]) => sel.includes("[data-kit='list-row-title']") && /white-space: (normal|pre-wrap)/.test(body))).toEqual([]);
});

test('a_tests_cards_long_command_wraps_inside_the_card_on_a_phone_v1_801', () => {
  // v1 #801: the tests card's command ran past the card at phone widths. v2 wraps it (design/layout.md: content text,
  // `overflow-wrap: anywhere` for code): the text column may shrink (min-width: 0) and the command breaks anywhere,
  // in the browser and both phone looks; no sheet makes it one line. Seen at 320/390 in target/r295/.
  const rules = everyRule();
  const on = (sel: string): string[] => rules.filter(([, s]) => s.split(',').some((x) => x.trim() === sel)).map(([, , b]) => b);
  expect(on("[data-kit='cb-tests-text']").some((b) => /min-width: 0/.test(b))).toBe(true);
  expect(on("[data-kit='cb-tests-cmd'] code").some((b) => /overflow-wrap: anywhere/.test(b))).toBe(true);
  expect(rules.filter(([, s, b]) => s.includes("[data-kit='cb-tests-cmd']") && /white-space: (nowrap|pre)\s*(;|$)/.test(b))).toEqual([]);
});
