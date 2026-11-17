// The side panel's place row (v1 panel.js .pn-place): over Changes, PR and Shell, not over Watch, Preview or a pane
// pinned beside tasks (/tmp/qa-keep/stuff-parity.md "Still different" 4: v2 showed it over a pane).
import { expect, test } from 'vitest';
import { showsPlace } from '../src/side/place.ts';

test('the_place_row_shows_over_the_tasks_folder_panes_and_not_over_a_page_of_its_own_as_v1', () => {
  expect(['changes', 'pr', 'shell'].map((p) => showsPlace(p as never))).toEqual([true, true, true]);
  expect(['watch', 'preview', 'tab'].map((p) => showsPlace(p as never))).toEqual([false, false, false]);
});
