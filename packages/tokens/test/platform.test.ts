// The phone apps' token sets (ADR-0065 §5, design/mobile.md §A.8).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { CSS_OUT, renderPlatforms } from '../src/build.ts';
import { renderPlatform, springLinear } from '../src/render-platform.ts';

const A8 = ['tap-min', 'bar-h', 'bar-h-large', 'nav-h', 'nav-rail-w', 'row-h-1', 'row-h-2', 'row-h-3', 'inset', 'radius-xs', 'radius-sm', 'radius-md', 'radius-lg', 'radius-xl', 'radius-sheet', 'radius-dialog', 'font-ui', 'surface-c0', 'surface-c1', 'surface-c2', 'surface-c3', 'surface-c4', 'on-surface', 'on-surface-var', 'outline', 'outline-var', 'state-hover', 'state-focus', 'state-pressed', 'state-drag', 'elev-1', 'elev-2', 'elev-3', 'ease-standard', 'ease-emph', 'ease-emph-in', 'ease-emph-out', 'dur-short', 'dur-medium', 'dur-long', 'swipe-line', 'grabber-w', 'grabber-h', 'safe-top', 'safe-right', 'safe-bottom', 'safe-left'];

test('every §A.8 token is set for each look, under its :root[data-platform] only', () => {
  for (const [look, css] of renderPlatforms()) {
    for (const name of A8) assert.match(css, new RegExp(`--${name}: `), `${look} lacks --${name}`);
    // every rule is scoped to the look: no bare :root that would reach the browser
    for (const m of css.matchAll(/^\s*([^@\s/][^\n{]*)\{\s*$/gm)) assert.ok(m[1]?.includes(`[data-platform='${look}']`), `${look}: unscoped rule ${m[1]}`);
  }
  assert.match(renderPlatforms().find(([l]) => l === 'android')?.[1] ?? '', /--fab: 56px;/);
  assert.match(renderPlatforms().find(([l]) => l === 'ios')?.[1] ?? '', /--tap-min: 44px;/);
});

test('the browser stylesheet carries no platform token', () => {
  assert.doesNotMatch(readFileSync(CSS_OUT, 'utf8'), /data-platform|--tap-min|--surface-c0/);
});

test('a token without its spec is refused', () => {
  assert.throws(() => renderPlatform('android', { static: { 'tap-min': { value: '48px' } } }), /cite its spec/);
  assert.throws(() => renderPlatform('ios', { static: { x: { value: '1px', spec: 's' } }, themed: { x: { light: '#fff', dark: '#000', spec: 's' } } }), /defined twice/);
});

test('a spring lands exactly on 1 and a bouncy one overshoots', () => {
  const pts = (s: string): number[] => (/linear\((.*)\)/.exec(s)?.[1] ?? '').split(', ').map(Number);
  const smooth = pts(springLinear(0.5, 0));
  const snappy = pts(springLinear(0.5, 0.15));
  assert.equal(smooth[0], 0);
  assert.equal(smooth.at(-1), 1);
  assert.ok(smooth.every((y, i) => i === 0 || y >= (smooth[i - 1] ?? 0)), 'bounce 0 never goes back');
  assert.ok(Math.max(...snappy) > 1, 'bounce 0.15 overshoots');
  assert.ok(Math.abs(1 - (smooth.at(-2) ?? 0)) < 0.01, 'sampled to its settle time: no jump at the end');
  assert.throws(() => springLinear(0, 0));
});

test('a platform token reuses a browser name only where §A.8 means to override it', () => {
  const browser = new Set([...readFileSync(CSS_OUT, 'utf8').matchAll(/(--[a-z0-9-]+):/g)].map((m) => m[1]));
  const meant = new Set(['--bar-h', '--radius-xs', '--radius-sm', '--radius-md', '--radius-lg', '--radius-xl']);
  for (const [look, css] of renderPlatforms()) {
    const names = new Set([...css.matchAll(/(--[a-z0-9-]+):/g)].map((m) => m[1] ?? ''));
    const clash = [...names].filter((n) => browser.has(n) && !meant.has(n));
    assert.deepEqual(clash, [], `${look} redefines browser tokens`);
  }
});

test('each look restates the kit layer order first', () => {
  for (const [, css] of renderPlatforms()) assert.match(css, /^\/\*[^]*?\*\/\n\/\*[^]*?\*\/\n@layer reset, tokens, kit, plugins;\n/);
});

test('the swipe numbers the kit swipe math mirrors (packages/ui swipe-math.ts) stay the spec numbers', () => {
  const css = (look: string): string => renderPlatforms().find(([l]) => l === look)?.[1] ?? '';
  // iOS: Mail's 74 pt action and its full swipe past half the row; Android: v1's third of the row
  assert.match(css('ios'), /--swipe-action-w: 74px;/);
  assert.match(css('ios'), /--swipe-line: 0\.5;/);
  assert.match(css('android'), /--swipe-line: 0\.33;/);
  // step 4's row and surface tokens exist in both looks
  for (const look of ['android', 'ios']) for (const name of ['row-lead', 'row-lead-gap', 'row-pad-y', 'chip-h', 'chip-pad', 'chip-icon', 'card-pad', 'banner-pad', 'refresh-spinner', 'empty-icon', 'button-h', 'button-pad']) assert.match(css(look), new RegExp(`--${name}: `), `${look} --${name}`);
});
