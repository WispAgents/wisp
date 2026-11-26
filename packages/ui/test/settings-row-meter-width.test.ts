// UI QA r1: a Meter as a settings row's control (Sandboxes' quota) drew 0px wide. The Meter grows into the room it's
// given (flex: 1 1 auto; min-width: 0) and the row's control box is sized by what's in it, so there was none. In a
// row's control it keeps a width of its own and doesn't shrink away; the Board's meter (not in a row) is left alone.
import { expect, test } from 'vitest';
import CSS from '../src/css/composites.css?raw';
import SETTINGS from '../src/css/settings.css?raw';

test('a_meter_in_a_settings_row_control_has_a_width_of_its_own', () => {
  const body = CSS.match(/\[data-kit='settings-row-control'\]\s*>\s*\[data-kit='meter'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(body).toMatch(/flex:\s*0 0 auto/);
  const size = body.match(/(?:^|[\s;])inline-size:\s*([0-9.]+)rem/)?.[1];
  expect(Number(size)).toBeGreaterThan(0);
  expect(body).toMatch(/max-inline-size:\s*100%/);
  // no narrow layout sets it back to nothing
  expect(CSS).not.toMatch(/\[data-kit='settings-row-control'\][^{]*\[data-kit='meter'\][^{]*\{[^}]*inline-size:\s*0/);
});

test('the_meter_itself_still_grows_where_it_isnt_in_a_row', () => {
  const body = SETTINGS.match(/\[data-kit='meter'\]\s*\{([^}]*)\}/)?.[1] ?? '';
  expect(body).toMatch(/flex:\s*1 1 auto/);
});
