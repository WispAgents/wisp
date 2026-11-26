// UI QA r1: a Code block's `lines` caps its height (a long setup log under the page's next step); the rest scrolls
// inside it. Other blocks keep growing with what they hold.
import { expect, test } from 'vitest';
import CSS from '../src/css/markdown.css?raw';

test('a_code_block_with_lines_is_capped_and_scrolls', () => {
  const body = CSS.match(/\[data-kit='code-pre'\]\[data-lines\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(body).toMatch(/max-block-size:\s*calc\(var\(--k-lines\) \* 1lh/);
  expect(body).toMatch(/overflow:\s*auto/);
  const plain = CSS.match(/\[data-kit='code-pre'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(plain).not.toMatch(/max-block-size|max-height/);
});
