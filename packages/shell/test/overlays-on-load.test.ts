// QA round 4 desktop #2 (round 3 #9 widened): a reload brought back the Machines dialog and the Settings sheet as it
// had ⌘K. v1 kept only the open task in its URL (#/s/<id>): no overlay ever came back. One rule for every overlay: a
// reload, or the app reopening its last place, brings back the place, never what was open over it. A link opened on
// purpose with `?o=` (a shared link, the QA gates' deep links) still opens its overlay.
import { expect, test } from 'vitest';
import { keepsOverlaysOnLoad } from '../src/frame/overlays-on-load.ts';

test('a_reload_or_a_reopened_last_place_brings_back_no_overlay_and_a_link_opened_on_purpose_does', () => {
  expect(keepsOverlaysOnLoad({ navigation: 'reload', search: '?o=dialog:machines.dialog:{}' })).toBe(false);
  expect(keepsOverlaysOnLoad({ navigation: 'reload', search: '' })).toBe(false);
  // `/` with nothing in it: the last place, restored from what was saved (any overlay in it is dropped)
  expect(keepsOverlaysOnLoad({ navigation: 'navigate', search: '' })).toBe(false);
  expect(keepsOverlaysOnLoad({ navigation: 'navigate', search: '?o=viewer:shell-settings.sheet:{"pane":"cloud"}' })).toBe(true);
  // Back and Forward into the app keep what that entry held
  expect(keepsOverlaysOnLoad({ navigation: 'back_forward', search: '?o=dialog:shell.palette' })).toBe(true);
  // unknown (an old WebView): as a reload
  expect(keepsOverlaysOnLoad({ navigation: undefined, search: '?o=dialog:shell.palette' })).toBe(false);
});
