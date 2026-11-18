/// <reference types="vite/client" />
// The kit's class mirror (build/kit-classes.ts, ADR-0104): bundles give every `data-kit` element a `k-` class and the
// kit's rules `.k-` selectors, which Blink buckets by class instead of trying all ~3,000 `[data-kit=…]` rules on every
// element at every style recalc (perf audit 2026-11-18 #5: style time per interaction down 60–80 % at 4×).
import { describe, expect, test } from 'vitest';
import { applyEdits, cssClasses, jsxEdits, KEEP_ATTR } from '../build/kit-classes.ts';

const mirror = (code: string): string => applyEdits(code, jsxEdits(code, 'x.tsx'), 'x.tsx').code;

describe('JSX: every data-kit element gets its k- class', () => {
  test('a static data-kit on an element_kit_classes_static', () => {
    expect(mirror('const a = <div data-kit="menu-item" data-part="x">hi</div>;')).toBe('const a = <div data-kit="menu-item" class="k-menu-item" data-part="x">hi</div>;');
  });
  test('an expression: the class follows the same expression_kit_classes_expr', () => {
    expect(mirror("const a = <button data-kit={on() ? 'button' : 'icon-button'} />;")).toBe("const a = <button data-kit={on() ? 'button' : 'icon-button'} class={`k-${on() ? 'button' : 'icon-button'}`} />;");
    expect(mirror('const a = <span data-kit={`${props.kit}-item`} />;')).toBe('const a = <span data-kit={`${props.kit}-item`} class={`k-${`${props.kit}-item`}`} />;');
  });
  test('Dynamic hands class to the element it draws; SVG elements too_kit_classes_dynamic_svg', () => {
    expect(mirror('<Dynamic component={tag} data-kit="text" />')).toBe('<Dynamic component={tag} data-kit="text" class="k-text" />');
    expect(mirror('<g data-kit="spirit-eyes" />')).toBe('<g data-kit="spirit-eyes" class="k-spirit-eyes" />');
  });
  test('a kit component (which takes no class) and an element that has one are left alone_kit_classes_skip', () => {
    expect(mirror('<Box data-kit="x" />')).toBe('<Box data-kit="x" />');
    expect(mirror('<div data-kit="x" class="y" />')).toBe('<div data-kit="x" class="y" />');
  });
  test("a props object spread onto an element ('data-kit': …) gets a class key_kit_classes_object", () => {
    expect(mirror("const attrs = () => ({ 'data-kit': 'anchored', role: 'dialog' });")).toBe("const attrs = () => ({ 'data-kit': 'anchored', class: 'k-anchored', role: 'dialog' });");
  });
  test('the sourcemap keeps every original column: a token after an insertion maps back to where it was_kit_classes_map', () => {
    const code = 'const a = 1;\nconst b = <i data-kit="x" id="after" />;\nconst c = 3;';
    const { map } = applyEdits(code, jsxEdits(code, 'x.tsx'), 'x.tsx');
    const m = JSON.parse(map) as { mappings: string; sources: string[] };
    expect(m.sources).toEqual(['x.tsx']);
    // line 2 has two segments: column 0, and the column after `data-kit="x"` (generated +14 for ` class="k-x"`)
    expect(m.mappings.split(';')).toHaveLength(3);
    expect(m.mappings.split(';')[1]?.split(',')).toHaveLength(2);
  });
});

describe('CSS: [data-kit=x] becomes .k-x', () => {
  test('quoted, unquoted and inside :where() / :not()_kit_classes_css', () => {
    expect(cssClasses("[data-kit='menu'] > [data-kit=\"menu-item\"]:not([data-kit=menu-sep]) { a: b }")).toBe('.k-menu > .k-menu-item:not(.k-menu-sep) { a: b }');
    expect(cssClasses(":where([data-kit='stack'], [data-kit='inline'])[data-align='start'] {}")).toBe(":where(.k-stack, .k-inline)[data-align='start'] {}");
  });
  test('the kits written outside JSX, prefixes and presence stay attributes_kit_classes_keep', () => {
    expect(cssClasses("[data-kit='term-row'] {} [data-kit^='spirit-'] {} [data-kit] {}")).toBe("[data-kit='term-row'] {} [data-kit^='spirit-'] {} [data-kit] {}");
  });
  test('every kit sheet: after the rewrite only KEEP_ATTR kits are left as [data-kit=…]_kit_classes_sheets', () => {
    const sheets = import.meta.glob<string>('../src/css/**/*.css', { query: '?raw', import: 'default', eager: true });
    const left = new Set<string>();
    for (const css of Object.values(sheets)) for (const m of cssClasses(css).matchAll(/\[data-kit=(['"]?)([\w-]+)\1\]/g)) left.add(m[2] ?? '');
    for (const k of left) expect(KEEP_ATTR.has(k), k).toBe(true);
  });
});

// Elements whose data-kit isn't written by JSX get no class: their kit must be in KEEP_ATTR, or its rules would stop
// matching them in the bundles (tests and the source still read data-kit, so nothing else would notice).
test('every data-kit written outside JSX is in KEEP_ATTR, and each KEEP_ATTR kit still has such a writer_kit_classes_guard', () => {
  const code = import.meta.glob<string>(['../src/**/*.ts', '../src/**/*.tsx', '../../shell/src/**/*.ts', '../../shell/src/**/*.tsx', '../../../plugins/*/ui/src/**/*.ts', '../../../plugins/*/ui/src/**/*.tsx'], { query: '?raw', import: 'default', eager: true });
  const imperative = new Set<string>();
  for (const [file, src] of Object.entries(code)) {
    for (const m of src.matchAll(/setAttribute\(\s*['"]data-kit['"]\s*,\s*['"]([\w-]+)['"]/g)) imperative.add(m[1] ?? '');
    for (const m of src.matchAll(/dataset\.kit\s*=\s*['"]([\w-]+)['"]/g)) imperative.add(m[1] ?? '');
    // HTML built as a string (`<div data-kit="term-row">`): only in .ts files, where it can't be JSX
    if (file.endsWith('.ts')) for (const m of src.matchAll(/<[a-z][^<>]*\sdata-kit=\\?["']([\w-]+)\\?["']/g)) imperative.add(m[1] ?? '');
  }
  for (const k of imperative) expect(KEEP_ATTR.has(k), `${k}: written outside JSX, so add it to KEEP_ATTR`).toBe(true);
  for (const k of KEEP_ATTR) expect(imperative.has(k), `${k}: no imperative writer left, drop it from KEEP_ATTR`).toBe(true);
});
