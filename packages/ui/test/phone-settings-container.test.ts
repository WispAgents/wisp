// QA phone r3 #1: on the phone apps Settings → Agents squeezed Claude Code's row: its name gone, "2.1.326 · 4 models"
// one character per line, "Update to 2.1.327" over "Sign out". A row's control drops under its words in a narrow
// `settings` container (composites.css), and the phone settings pane wasn't one, so no phone row ever did.
import { expect, test } from 'vitest';
import CSS from '../src/css/phone-pages.css?raw';

test('the_phone_settings_pane_is_a_settings_container_so_wide_controls_drop_under_their_words', () => {
  const rule = CSS.match(/\[data-kit='phone-settings-pane'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(rule).toMatch(/container:\s*settings\s*\/\s*inline-size/);
});
