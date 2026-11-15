/// <reference types="vite/client" />
// The phone apps' sheets (src/css/platform/*.css, ADR-0065 §5) follow the kit's CSS rules, with one
// difference: they restate the layer order (they load on their own) and every rule is scoped to its look.
import { expect, test } from 'vitest';
import { platformRules, platformSheetProblems, type PlatformSheetRule } from '@wisp/tokens/css-rules';

const files = import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true });
const sheets = Object.entries(files).map(([path, raw]) => ({ look: path.replace(/^.*\/|\.css$/g, ''), raw, css: raw.replace(/\/\*[^]*?\*\//g, '') }));
/** Every rule as [selector, body, the at-rules it sits in]. */
const rules = (css: string): Array<[string, string, string]> => platformRules(css).map((r) => [r.selector, r.body, r.at]);

test('there_is_a_sheet_per_phone_look_with_the_frame_and_navigation_variants', () => {
  expect(sheets.map((s) => s.look).sort()).toEqual(['android', 'ios']);
  for (const s of sheets) for (const kit of ['frame', 'frame-top', 'nav-bar', 'nav-item', 'app-bar', 'app-bar-large']) expect(s.css, `${s.look} ${kit}`).toContain(`[data-kit='${kit}']`);
  expect(sheets.find((s) => s.look === 'android')?.css).toContain("[data-kit='fab']");
});

// every ban below is @wisp/tokens' platformSheetProblems, which `npm run check`'s css rule runs too (rounds 288, 291)
const banned = (rule: PlatformSheetRule): string[] => sheets.flatMap((s) => platformSheetProblems(s.look, s.raw).filter((p) => p.kind === rule).map((p) => `${p.line}: ${p.message}`));

test('every_platform_rule_sits_in_layer_kit_under_its_own_look', () => {
  for (const s of sheets) expect(rules(s.css).length, s.look).toBeGreaterThan(20);
  expect(banned('platform-scope')).toEqual([]);
});

test('platform_px_come_from_tokens_and_queries_only_from_the_size_classes', () => {
  expect(banned('platform-px')).toEqual([]);
  expect(banned('platform-query')).toEqual([]);
});

test('platform_sheets_keep_the_kit_rules_no_has_no_is_no_colours_no_wrapping', () => {
  for (const rule of ['has-is', 'colour', 'wrap', 'layout', 'class', 'sibling-universal'] as const) expect(banned(rule), rule).toEqual([]);
});

test('nav_labels_and_bar_titles_never_wrap', () => {
  // the nav's label and the title button's text are [data-text='control'] (content.css: nowrap, ellipsis)
  for (const s of sheets) {
    const titles = rules(s.css).filter(([sel]) => /\[data-kit='app-bar-title'\]$/.test(sel));
    expect(titles.some(([sel, body]) => sel.endsWith(`:root[data-platform='${s.look}'] [data-kit='app-bar-title']`) && /white-space: nowrap/.test(body) && /text-overflow: ellipsis/.test(body)), s.look).toBe(true);
  }
  expect(banned('title-wraps')).toEqual([]);
});

test('a_feed_rows_time_and_pills_each_have_their_own_line_in_one_trailing_column', () => {
  // r285: the time sat on the status pill; now line 1 holds the time, line 2 the pills, both in column 3, the title stops before it
  for (const s of sheets) {
    const R = `:root[data-platform='${s.look}'] [data-kit='list-row'][data-layout='feed']`;
    const body = (sel: string): string => rules(s.css).filter(([x]) => x.split(',').some((one) => one.trim() === sel)).map(([, b]) => b).join(';');
    expect(body(`${R} > [data-kit='time']`), s.look).toMatch(/grid-column: 3; grid-row: 1;/);
    expect(body(`${R} > [data-kit='time']`), s.look).not.toMatch(/position: absolute/);
    expect(body(`${R}[data-time][data-row-lines] > [data-kit='list-row-pills']`), s.look).toMatch(/grid-column: 3; grid-row: 2;/);
    expect(body(`${R} > [data-kit='list-row-title']`), s.look).toMatch(/grid-column: 2;/);
  }
});

test('a_crew_face_rows_title_starts_at_the_same_x_as_a_dot_rows', () => {
  // r291: the browser's 10 px dot column let a crew member's 20 px face run into the title; on the phones the leading
  // column is one fixed width (the face's, --size-avatar-sm), the dot, the face and the mark icons centred in it, so
  // the title (column 2) starts at one x on every row, whatever leads it
  for (const s of sheets) {
    const R = `:root[data-platform='${s.look}'] [data-kit='list-row'][data-layout='feed']`;
    const body = (sel: string): string => rules(s.css).filter(([x]) => x.split(',').some((one) => one.trim() === sel)).map(([, b]) => b).join(';');
    expect(body(R), s.look).toMatch(/grid-template-columns: var\(--size-avatar-sm\) minmax\(0, 1fr\) auto var\(--k-feed-done,\);/);
    for (const lead of ["[data-kit='dot']", "[data-kit='face']", "[data-kit='icon'][data-part~='list.row.mark']"])
      expect(body(`${R} > ${lead}`), `${s.look} ${lead}`).toMatch(/grid-column: 1; grid-row: 1; justify-self: center;/);
  }
});
