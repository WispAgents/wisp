// The side panel's Shell (audit 2026-11-19): coming back to a shell that was started at another width (another window,
// the panel dragged since) attached at the shell's old size, so the prompt ran past the pane's right edge and was cut.
// v1 term-view.js fit the screen and told the pty: the pane says its own size when the hello's differs.
import { expect, test } from 'vitest';
import { resizeOnHello } from '../src/side/shell-size.ts';

test('a_shell_attached_at_another_size_is_told_the_panes_size', () => {
  expect(resizeOnHello({ cols: 140, rows: 40 }, { cols: 58, rows: 46 })).toEqual({ cols: 58, rows: 46 });
  expect(resizeOnHello({ cols: 58, rows: 46 }, { cols: 58, rows: 46 }), 'the same size: nothing to say').toBeUndefined();
  expect(resizeOnHello({ cols: 140, rows: 40 }, undefined), 'not measured yet: the terminal says it once it has').toBeUndefined();
});
