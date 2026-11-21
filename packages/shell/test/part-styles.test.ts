// A trusted theme's partStyles → the CSS the shell writes into @layer plugins (ADR-0007). Every declaration the
// kernel's allow-list refuses is dropped here too (same part-styles.json), and nothing a value or a part name
// holds can close its rule, open another, import a sheet, win with !important or fetch from a remote host.
import { expect, test } from 'vitest';
import { partOk, partStylesCss, propertyOk, valueProblem } from '../src/theme-styles/part-styles.ts';
import allow from '../src/theme-styles/part-styles.json' with { type: 'json' };

const css = (partStyles: unknown, plugin = 'user.theme'): string => partStylesCss([{ plugin, partStyles }]);

test('a part becomes [data-part~="…"] inside @layer plugins', () => {
  expect(css({ 'composer.send': { background: 'hotpink', 'border-radius': '999px' } })).toBe(
    '@layer plugins {\n/* user.theme */\n[data-part~="composer.send"] { background: hotpink; border-radius: 999px; }\n}\n',
  );
});

test('token overrides go on :root, custom properties only', () => {
  const out = css({ ':root': { '--accent': '#e0457b', color: 'red' } });
  expect(out).toContain(':root { --accent: #e0457b; }');
  expect(out).not.toContain('color: red');
});

test('@dark and @light apply in their theme, and under the OS while the theme follows the system', () => {
  const out = css({ '@dark': { ':root': { '--accent': '#ff7aa8' }, 'ui.button': { color: 'white' } }, '@light': { 'ui.button': { color: 'black' } } });
  expect(out).toContain(':root[data-theme="dark"] { --accent: #ff7aa8; }');
  expect(out).toContain('@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { --accent: #ff7aa8; } }');
  expect(out).toContain(':root[data-theme="dark"] [data-part~="ui.button"] { color: white; }');
  expect(out).toContain(':root[data-theme="light"] [data-part~="ui.button"] { color: black; }');
  expect(out).toContain('@media (prefers-color-scheme: light) { :root:not([data-theme="dark"]) [data-part~="ui.button"] { color: black; } }');
});

test('injections in values are dropped, declaration by declaration', () => {
  const bad = [
    'red; } body { display: none',
    'red } * { color: red',
    'red !important',
    'url(https://evil.example/x.png)',
    "url( 'http://evil.example/x' )",
    'image-set("https://evil.example/a.png" 1x)',
    'red /* */',
    '@import url(x)',
    '"unbalanced',
    '</style><script>alert(1)</script>',
    'a\\7d b',
    'red\n}',
    '',
    'x'.repeat(201),
  ];
  for (const v of bad) expect(valueProblem(v), v).toBeDefined();
  const out = css({ 'composer.send': Object.fromEntries([...bad.map((v, i) => [`--x${i}`, v]), ['color', 'teal']]) });
  expect(out).toBe('@layer plugins {\n/* user.theme */\n[data-part~="composer.send"] { color: teal; }\n}\n');
  expect(valueProblem('url(data:image/png,AAAA)')).toBeUndefined();
  expect(valueProblem('url("data:image/svg+xml;utf8,x")')).toBeDefined(); // a ; can't pass, even in a data URL
});

test('part names, properties and groups are checked', () => {
  expect(partOk('composer.send') && partOk('ui.button.label')).toBe(true);
  for (const p of ['composer', 'Composer.send', 'x"] body [y.z', 'a..b', '.a', 'a.b c']) expect(partOk(p), p).toBe(false);
  expect(propertyOk('position', false)).toBe(false);
  expect(propertyOk('--ok', true) && propertyOk('background', false)).toBe(true);
  expect(propertyOk('background', true)).toBe(false);
  expect(css({ 'x"] body [y.z': { color: 'red' }, '@sepia': { 'a.b': { color: 'red' } }, '@dark': { '@light': { 'a.b': { color: 'red' } } } })).toBe('');
});

test('nothing to apply is no stylesheet; a plugin id that could end its comment is not written', () => {
  expect(partStylesCss([])).toBe('');
  expect(css({ 'a.b': { position: 'fixed' } })).toBe('');
  expect(css({ 'a.b': { color: 'red' } }, 'user.x */ body {')).toContain('/* plugin */');
});

test('the shell and the kernel read one allow-list', () => {
  expect(allow.properties).toContain('background');
  expect(allow.groups).toEqual(['@light', '@dark']);
});
