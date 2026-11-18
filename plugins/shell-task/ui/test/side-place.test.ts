// The side panel's place row (v1 panel.js .pn-place). UX 2026-11-18 desktop #5: "This computer" repeated under every
// tab. The machine shows only in Shell's header, and only when the task isn't on this computer; the row's ⋯ (copy
// path, branch, open in editor or terminal app) moved to the panel's head so it stays one press away.
import { expect, test } from 'vitest';
import { showsPlace } from '../src/side/place.ts';

test('the_place_row_shows_only_over_shell_and_only_for_a_task_on_another_computer', () => {
  expect(['changes', 'pr', 'shell', 'watch', 'preview', 'tab', 'term'].map((p) => showsPlace(p as never, false))).toEqual([false, false, false, false, false, false, false]);
  expect(['changes', 'pr', 'shell', 'watch', 'preview', 'tab', 'term'].map((p) => showsPlace(p as never, true))).toEqual([false, false, true, false, false, false, false]);
});
