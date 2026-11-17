// Closing the parity audit's leftovers (/tmp/qa-keep/stuff-parity.md "Still different" 3 and 6).
import { expect, test } from 'vitest';
import { appAddress, openedByPress } from '../src/opened.ts';

// what wisp.grid-client's previewAddresses entry answers inside a box (ADR-0064: its boxPreviewUrl with the grid's template)
const grid = (url: string): string | undefined => {
  const u = new URL(url);
  return u.hostname === '127.0.0.1' ? `https://p${u.port}--ada.grid.example${u.pathname}` : undefined;
};

test('an_app_launched_from_a_page_in_a_grid_box_loads_at_the_grids_preview_address_as_v1_did', () => {
  // v1 launchApp: isLoopback(location.href) ? url : boxUrl(url, S.config.preview)
  expect(appAddress('http://127.0.0.1:47639/', 'http://localhost:5180/view/stuff.page', [grid])).toBe('http://127.0.0.1:47639/');
  expect(appAddress('http://127.0.0.1:47639/', 'https://ada.grid.example/view/stuff.page', [grid])).toBe('https://p47639--ada.grid.example/');
  // nothing maps it (no grid, another computer's app): nowhere, so the page copies the address and says where it runs
  expect(appAddress('http://127.0.0.1:47639/', 'https://ada.grid.example/view/stuff.page', [() => undefined])).toBeUndefined();
  expect(appAddress('http://127.0.0.1:47639/', 'https://ada.grid.example/view/stuff.page', [])).toBeUndefined();
});

test('my_stuff_puts_the_caret_in_its_search_only_when_a_press_opened_it_not_after_a_reload', () => {
  expect(openedByPress({ userActivation: { isActive: true } })).toBe(true);
  expect(openedByPress({ userActivation: { isActive: false } })).toBe(false);
  expect(openedByPress({})).toBe(true);
});
