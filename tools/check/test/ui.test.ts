import assert from 'node:assert/strict';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { kitCssProblems, scanCss, splitTop, subjectOf } from '../../../packages/tokens/src/css-rules.ts';
import { kitScale } from '../core/tokens.ts';
import { css, endsInIsList, hasOnLargeBox, importantAllowed } from '../rules/css.ts';
import { BREAKPOINT_PX, PX_SCALE } from '../../../packages/tokens/src/generated.ts';
import { i18n } from '../rules/i18n.ts';
import { state } from '../rules/state.ts';
import { fixture, messages, run, writeTree } from './helpers.ts';

test('css: kit CSS on the token scale, no slow selectors, no !important, nothing wraps; no CSS outside the kit', async () => {
  assert.deepEqual(await run(css, fixture('css')), [
    'packages/ui/src/kit.css:10 css', // ends in :is()
    'packages/ui/src/kit.css:12 css', // !important
    'packages/ui/src/kit.css:20 css', // flex-wrap: wrap on a toolbar (line 22's !important has a reason)
    'packages/ui/src/kit.css:24 css', // flex-flow: row wrap on a Wrap's child, not on the Wrap
    'packages/ui/src/kit.css:26 css', // white-space: normal on a button
    'packages/ui/src/kit.css:27 css', // text-wrap: balance on control text
    'packages/ui/src/kit.css:3 css', // 13px
    'packages/ui/src/kit.css:30 css', // pre-line on a list-row title, inside a media query
    'packages/ui/src/kit.css:5 css', // 721px media query
    'packages/ui/src/kit.css:6 css', // .stage:has
    'packages/ui/src/kit.css:7 css', // body:has
    'packages/ui/src/kit.css:8 css', // bare :has
    'plugins/git/ui/src/panel.css:1 css', // a .css file outside the kit
    'plugins/git/ui/src/panel.tsx:1 css', // CSS import
    'plugins/git/ui/src/panel.tsx:3 css', // class=
    'plugins/git/ui/src/panel.tsx:4 css', // className=
    'plugins/git/ui/src/panel.tsx:5 css', // style string
    'plugins/git/ui/src/panel.tsx:6 css', // style with a real property
    'plugins/git/ui/src/panel.tsx:8 css', // style from a variable: can't be checked
    'plugins/git/ui/src/theme.ts:1 css', // .scss?inline import
  ]);
});

test('css_a_raw_px_media_query_and_a_flex_wrap_outside_wrap_fail_the_check', async () => {
  // round 244: `npm run check` passed `@container launcher (max-width: 600px)` (600 is on the scale, not a breakpoint)
  // and `[data-part='runner.account-wrap'] { flex-wrap: wrap }` (a data-part named …wrap), while the kit's vitest
  // refused both. Both gates now run @wisp/tokens' css-rules.ts; each of these lines fails `npm run check`.
  const kit = [
    '@layer kit {',
    '  @media (max-width: 560px) { [data-kit="a"] { gap: 8px; } }', // 2: 560 is on the scale, not a breakpoint
    '  @container launcher (max-width: 600px) { [data-kit="a"] { gap: 8px; } }', // 3: same, in a container query
    '  @media (max-width: 720px) { [data-kit="a"] { gap: 8px; } }', // 4: a breakpoint: fine
    "  [data-kit='seg'][data-part='runner.account-wrap'] { flex-wrap: wrap; }", // 5: a data-part named …wrap
    "  [data-kit='seg'] { flex-flow: row wrap; }", // 6: no Wrap at all
    "  [data-kit='seg'][data-layout='wrap'] { gap: 8px; }", // 7: a Wrap group styling its gap: fine
    "  [data-kit='wrap'], [data-layout='wrap'] { flex-wrap: wrap; }", // 8: Wrap's own rule: fine
    "  [data-kit='a'], [data-layout='wrap'] { flex-wrap: wrap; }", // 9: one selector of the list isn't Wrap
    '  [data-kit="a"] { width: 720px; }', // 10: a breakpoint is not a length
    "  [data-kit='seg']:not([data-layout='wrap']) { flex-wrap: wrap; }", // 11: names Wrap only to exclude it
    "  [data-kit='a']:where([data-layout='wrap'], [data-kit='x'])[data-open] { flex-wrap: wrap; }", // 12: Wrap inside a list
    "  [data-layout='wrap']:not([data-kit='x']) { flex-wrap: wrap; }", // 13: a Wrap, narrowed: fine
    '}',
  ].join('\n');
  const root = writeTree(
    new Map([
      ['packages/tokens/package.json', '{"name":"@f/tokens"}'],
      ['packages/tokens/tokens/space.json', '{ "2": "8px" }'],
      ['packages/tokens/tokens/size.json', '{ "a": "560px", "b": "600px" }'],
      ['packages/tokens/tokens/breakpoint.json', '{ "phone": "720px" }'],
      ['packages/ui/package.json', '{"name":"@f/ui"}'],
      ['packages/ui/src/kit.css', kit],
    ]),
  );
  assert.deepEqual(await run(css, root), [2, 3, 5, 6, 9, 10, 11, 12].map((n) => `packages/ui/src/kit.css:${n} css`).sort());
  const sets = kitScale(root);
  assert.ok(sets);
  assert.deepEqual(
    kitCssProblems(kit, sets).map((p) => [p.line, p.kind]),
    [[2, 'px'], [3, 'px'], [5, 'layout'], [6, 'layout'], [9, 'layout'], [10, 'px'], [11, 'layout'], [12, 'layout']],
  );
  assert.equal(subjectOf("[data-kit='x'] > [data-layout='wrap']:hover"), "[data-layout='wrap']:hover");
  assert.equal(subjectOf('[data-kit="a b"]'), '[data-kit="a b"]');
});

test('css_the_checks_token_sets_are_the_kits_px_scale_and_breakpoints', () => {
  // packages/ui/test/css-rules.test.ts runs @wisp/tokens/css-rules with @wisp/tokens' PX_SCALE and BREAKPOINT_PX; the check reads
  // the token JSON. Equal sets, so the two gates can't disagree on a px.
  const sets = kitScale(fileURLToPath(new URL('../../..', import.meta.url)));
  assert.ok(sets);
  const sorted = (s: Iterable<number>): number[] => [...s].sort((a, b) => a - b);
  assert.deepEqual(sorted(sets.scale), sorted(PX_SCALE));
  assert.deepEqual(sorted(sets.breakpoints), sorted(BREAKPOINT_PX));
});

test('css: without token files the px check is skipped with one warning', async () => {
  const root = writeTree(new Map([['packages/ui/package.json', '{"name":"@f/ui"}'], ['packages/ui/src/a.css', '.a { gap: 13px; }']]));
  assert.deepEqual(await run(css, root), ['packages/tokens/tokens:1 warn:css']);
});

test('css: selector helpers', () => {
  assert.equal(hasOnLargeBox('body:has(dialog[open])'), true);
  assert.equal(hasOnLargeBox('html.dark:has(.x)'), true);
  assert.equal(hasOnLargeBox('[data-part="shell.frame"]:has(.x)'), true);
  assert.equal(hasOnLargeBox('.list > :has(.x)'), true);
  assert.equal(hasOnLargeBox('.row:has(> .chip)'), false);
  assert.equal(hasOnLargeBox('.row:not(.a):has(.chip)'), false);
  assert.equal(endsInIsList('.a > :is(.b, .c)'), true);
  assert.equal(endsInIsList('.a :where(.b)'), true);
  assert.equal(endsInIsList(':is(.a, .b) .c'), false);
  assert.equal(endsInIsList('.a:not(.b)'), false);
  assert.deepEqual(splitTop('.a, :is(.b, .c), [x=","]'), ['.a', ':is(.b, .c)', '[x=","]']);
  assert.equal(importantAllowed(['@layer reset', '[hidden]']), true);
  assert.equal(importantAllowed(['@layer kit', '@media (prefers-reduced-motion: reduce)', '.x']), true);
  assert.equal(importantAllowed(['@layer kit', '.x']), false);
});

test('css: the scanner keeps lines through comments, strings and nesting', () => {
  const { preludes, decls } = scanCss('/* a\n b */\n.a {\n  color: red;\n  &:hover { content: "x;\\\ny"; }\n}\n.b { gap: 4px }');
  assert.deepEqual(
    preludes.map((p) => [p.text, p.line]),
    [['.a', 3], ['&:hover', 5], ['.b', 8]],
  );
  assert.deepEqual(
    decls.map((d) => [d.prop, d.line, d.within.join(' / ')]),
    [['color', 4, '.a'], ['content', 5, '.a / &:hover'], ['gap', 8, '.b']],
  );
});

test('state: ui packages keep no module state, no fetch, no history/location, no bare timers', async () => {
  assert.deepEqual(await run(state, fixture('state')), [
    'plugins/git/ui/src/panel.tsx:1 state', // let
    'plugins/git/ui/src/panel.tsx:10 state', // window.location
    'plugins/git/ui/src/panel.tsx:11 state', // document.location
    'plugins/git/ui/src/panel.tsx:12 state', // setTimeout
    'plugins/git/ui/src/panel.tsx:13 state', // globalThis.setInterval
    'plugins/git/ui/src/panel.tsx:2 state', // var
    'plugins/git/ui/src/panel.tsx:6 state', // fetch
    'plugins/git/ui/src/panel.tsx:7 state', // window.fetch
    'plugins/git/ui/src/panel.tsx:8 state', // history
    'plugins/git/ui/src/panel.tsx:9 state', // location
  ]);
});

test('i18n: JSX text and text attributes in plugin ui are warnings', async () => {
  const found = await messages(i18n, fixture('quality'));
  assert.deepEqual(found, [
    'plugins/git/ui/src/panel.tsx:3 warn:i18n title="Branches": use t(\'key\')',
    'plugins/git/ui/src/panel.tsx:4 warn:i18n "Push now" in JSX: use t(\'key\')',
    'plugins/git/ui/src/panel.tsx:6 warn:i18n placeholder="Search branches": use t(\'key\')',
  ]);
});
