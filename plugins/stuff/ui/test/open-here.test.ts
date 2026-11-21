// Closing the parity audit's leftovers (/tmp/qa-keep/stuff-parity.md "Still different" 3 and 6).
import { expect, test } from 'vitest';
import { appAddress, openedByPress, opensHere } from '../src/opened.ts';

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

// audit C tasks #1: on the phone apps, a doc, a table or code opened the raw file in a new window (a native shell may
// show nothing at all). The phone screen hands in `show` now: everything but an app opens in wisp's reader and viewer;
// its apps still go over the whole screen (`launched`)
test('the_phone_screen_opens_docs_and_tables_in_wisp_and_its_apps_over_the_whole_screen_audit_c_1', () => {
  const show = (): void => {};
  const launched = (): void => {};
  for (const kind of ['doc', 'table', 'code', 'image', 'file', 'link']) expect(opensHere(kind, { show, launched }), kind).toBe(true);
  expect(opensHere('app', { show, launched })).toBe(false);
  // the desktop page: its apps in wisp's own window
  expect(opensHere('app', { show })).toBe(true);
  // no reader on the screen: a new window, as before
  expect(opensHere('doc', {})).toBe(false);
});
