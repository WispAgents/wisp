import assert from 'node:assert/strict';
import test from 'node:test';
import { css } from '../rules/css.ts';
import { messages, writeTree } from './helpers.ts';

test('css_check_flags_every_kit_sheet_rule_the_vitest_enforces', async () => {
  // round 265: the kit's vitest refused `:is(` in packages/ui/src/css/queued.css while `npm run check` passed it.
  // Both now run @wisp/tokens' kitSheetProblems; each rule below fails `npm run check` on a kit sheet.
  const bad = [
    '@layer kit {', // 1
    "  [data-kit='a']:is([data-x]) { gap: 8px; }", // 2: has-is
    "  [data-kit='a']:has([data-x]) { gap: 8px; }", // 3: has-is
    "  [data-kit='a'] > * + * { gap: 8px; }", // 4: sibling-universal
    "  [data-kit='a'] { color: #fff; }", // 5: colour
    '  .thing { gap: 8px; }', // 6: class
    "  [data-kit='a'] { white-space: normal; }", // 7: white-space-normal
    "  [data-kit='button'] { transform: translate(-50%); }", // 8: pressed-translate
    "  [data-kit='sh-title'] { line-clamp: 2; }", // 9: head-title-wraps
    "  [data-kit='a'] { gap: 13px; }", // 10: px
    "  [data-kit='a'] { flex-wrap: wrap; }", // 11: layout
    '  @layer x, y;', // 12: layer-order
    '}',
    "[data-kit='b'] { gap: 8px; }", // 14: layer-block
  ].join('\n');
  const root = writeTree(
    new Map([
      ['packages/tokens/package.json', '{"name":"@f/tokens"}'],
      ['packages/tokens/tokens/space.json', '{ "2": "8px" }'],
      ['packages/tokens/tokens/breakpoint.json', '{ "phone": "720px" }'],
      ['packages/ui/package.json', '{"name":"@f/ui"}'],
      ['packages/ui/src/css/index.css', '@layer reset, tokens, kit, plugins;\n'],
      ['packages/ui/src/css/bad.css', bad],
    ]),
  );
  const found = (await messages(css, root)).filter((m) => m.includes('bad.css'));
  const kinds = new Set(found.map((m) => /\[([\w-]+)\]/.exec(m)?.[1]));
  for (const k of ['has-is', 'sibling-universal', 'colour', 'class', 'white-space-normal', 'pressed-translate', 'head-title-wraps', 'px', 'layout', 'layer-order', 'layer-block']) assert.ok(kinds.has(k), `${k} not flagged: ${found.join('\n')}`);
  assert.ok(found.some((m) => /bad\.css:2\b.*:is\(/.test(m)), found.join('\n'));
  assert.ok(found.some((m) => /bad\.css:3\b.*:has\(/.test(m)), found.join('\n'));
  // each problem sits on its selector's own line
  for (const [line, kind] of [[4, 'sibling-universal'], [8, 'pressed-translate'], [9, 'head-title-wraps']] as const)
    assert.ok(found.some((m) => m.includes(`bad.css:${line} css [${kind}]`)), `${kind} not on line ${line}: ${found.join('\n')}`);
  // the index sheet itself is clean
  assert.deepEqual((await messages(css, root)).filter((m) => m.startsWith('packages/ui/src/css/index.css')), []);
});
