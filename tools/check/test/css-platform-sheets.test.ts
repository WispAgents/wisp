import assert from 'node:assert/strict';
import test from 'node:test';
import { css } from '../rules/css.ts';
import { messages, writeTree } from './helpers.ts';

test('css_check_flags_a_raw_px_and_a_non_size_class_query_in_a_platform_sheet', async () => {
  // rounds 288 and 291: a raw px in packages/ui/src/css/platform/*.css passed `npm run check` and failed only the
  // platform vitest. Both now run @wisp/tokens' platformSheetProblems.
  const sheet = [
    '@layer kit {', // 1
    "  :root[data-platform='ios'] [data-kit='chip'] { height: 32px; }", // 2: platform-px (on the kit scale, still raw)
    '  @media (max-width: 720px) {', // 3: platform-query
    "    :root[data-platform='ios'] [data-kit='a'] { gap: var(--space-2); }",
    '  }',
    '  @media (min-width: 600px) {', // 6: a size class: fine
    "    :root[data-platform='ios'] [data-kit='b'] { gap: var(--space-2); }",
    '  }',
    "  [data-kit='c'] { gap: var(--space-2); }", // 9: platform-scope
    '}',
  ].join('\n');
  const root = writeTree(
    new Map([
      ['packages/tokens/package.json', '{"name":"@f/tokens"}'],
      ['packages/tokens/tokens/space.json', '{ "2": "8px", "8": "32px" }'],
      ['packages/tokens/tokens/breakpoint.json', '{ "phone": "720px" }'],
      ['packages/ui/package.json', '{"name":"@f/ui"}'],
      ['packages/ui/src/css/platform/ios.css', sheet],
    ]),
  );
  const found = (await messages(css, root)).filter((m) => m.includes('platform/ios.css'));
  for (const [line, kind] of [[2, 'platform-px'], [3, 'platform-query'], [9, 'platform-scope']] as const)
    assert.ok(found.some((m) => m.includes(`ios.css:${line} css [${kind}]`)), `${kind} not on line ${line}: ${found.join('\n')}`);
  assert.ok(!found.some((m) => m.includes('ios.css:6')), found.join('\n'));
});
