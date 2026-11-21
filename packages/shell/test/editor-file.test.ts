// Audit A #10: a file link in an answer (the Files block's ↗, a code block's "math.js:1", a failing test's file) opened
// an empty Changes pane: it opens the file in the editor Settings names, at its line, the path taken from the task's
// folder when the answer gives it relative (v1 openInEditor).
import { expect, test } from 'vitest';
import { fileUrl } from '../src/editor.ts';

test('an_answers_file_link_opens_the_file_at_its_line_in_the_editor', () => {
  expect(fileUrl('', '/code/app', 'src/math.js', 4)).toBe('vscode://file/code/app/src/math.js:4');
  expect(fileUrl('cursor', '/code/app/', './math.js')).toBe('cursor://file/code/app/math.js');
  expect(fileUrl('vscode', '/code/app', '/abs/x.ts', 2)).toBe('vscode://file/abs/x.ts:2');
  // no folder to resolve a relative path, or no editor wisp knows: nothing to open (the caller falls back)
  expect(fileUrl('vscode', '', 'math.js')).toBeUndefined();
  expect(fileUrl('none', '/code/app', 'math.js')).toBeUndefined();
});
