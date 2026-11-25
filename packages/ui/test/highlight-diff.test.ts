// ```diff / ```patch fences: coloured a line at a time (added, removed, hunk, header), never by keyword.
import { expect, test } from 'vitest';
import { highlight } from '../src/content/highlight.ts';

const kinds = (code: string, lang = 'diff'): (string | null)[] => highlight(code, lang).map((t) => t.kind);

test('a removed line is one del token, with no keyword inside it', () => {
  const tokens = highlight('-  return x', 'diff');
  expect(tokens).toEqual([{ kind: 'del', text: '-  return x' }]);
  expect(tokens.some((t) => t.kind === 'k')).toBe(false);
});

test('file headers are not added or removed lines; @@ is a hunk', () => {
  expect(highlight('+++ b/x', 'diff')[0]?.kind).not.toBe('add');
  expect(highlight('--- a/x', 'patch')[0]?.kind).not.toBe('del');
  expect(kinds('@@ -1 +1 @@')).toEqual(['hunk']);
});

test('a whole diff: one token per changed line, the rest plain, and the text joins back', () => {
  const src = 'diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n const a = 1;\n-return a\n+return b\n';
  expect(highlight(src, 'diff').map((t) => t.text).join('')).toBe(src);
  expect(kinds(src).filter((k) => k !== null)).toEqual(['meta', 'meta', 'meta', 'hunk', 'del', 'add']);
  expect(kinds(src, 'PATCH').filter((k) => k !== null)).toEqual(['meta', 'meta', 'meta', 'hunk', 'del', 'add']);
});
