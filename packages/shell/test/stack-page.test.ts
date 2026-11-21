// F-phone-7: under 900 px a task opens as its own page with Back to the list (v1 body.stack), as plain rules.
import { expect, test } from 'vitest';
import { initialNav, layoutFor, navigate, type Nav, type SessionId } from '@wisp/client';
import { escLeavesLauncher, stackView } from '../src/frame/stack-page.ts';

const id = 's1' as SessionId;
const at = (width: number): Nav => initialNav(width);

test('under_900_px_the_list_is_the_page_and_a_tapped_task_replaces_it', () => {
  expect([layoutFor(899), layoutFor(900), layoutFor(390), layoutFor(320)]).toEqual(['stack', 'split', 'stack', 'stack']);
  const list = at(390);
  expect(stackView(list, false)).toEqual({ page: 'list', listBehind: true, idleHome: true, homeOn: false });
  const task = navigate(list, { t: 'openTask', id });
  expect(stackView(task, false)).toEqual({ page: 'main', listBehind: false, idleHome: false, homeOn: false });
  // Back (the page's ‹, the browser's Back, Esc) is back to the list
  expect(stackView(navigate(task, { t: 'back' }), false).page).toBe('list');
});

test('at_900_px_and_up_home_stays_built_beside_the_list', () => {
  const v = stackView(at(1440), false);
  expect(v.idleHome).toBe(false);
  expect(v.homeOn).toBe(true);
  expect(stackView(navigate(at(1440), { t: 'openTask', id }), false).idleHome).toBe(false);
});

test('new_task_on_a_phone_is_its_own_page_over_the_list_which_stays_laid_out_behind', () => {
  expect(stackView(at(390), true)).toEqual({ page: 'main', listBehind: true, idleHome: false, homeOn: true });
});

test('a_window_narrowed_under_900_px_with_a_task_open_shows_that_task_as_the_page', () => {
  const wide = navigate(at(1200), { t: 'openTask', id });
  const narrow = navigate(wide, { t: 'resize', width: 600 });
  expect(narrow.layout).toBe('stack');
  expect(stackView(narrow, false).page).toBe('main');
  expect(stackView(navigate(narrow, { t: 'resize', width: 1000 }), false).page).toBe('main'); // split shows both
});

test('esc_on_a_phones_new_task_page_goes_back_to_the_list_once_out_of_the_text_box', () => {
  const key = { key: 'Escape', defaultPrevented: false, typing: false };
  expect(escLeavesLauncher(at(390), true, key)).toBe(true);
  expect(escLeavesLauncher(at(390), true, { ...key, typing: true })).toBe(false); // the first Esc blurs the box
  expect(escLeavesLauncher(at(390), true, { ...key, defaultPrevented: true })).toBe(false); // a menu took it
  expect(escLeavesLauncher(at(390), false, key)).toBe(false);
  expect(escLeavesLauncher(at(1440), true, key)).toBe(false); // split: Home is the main pane, not a page
  expect(escLeavesLauncher(at(390), true, { ...key, key: 'Enter' })).toBe(false);
  const overlay = navigate(at(390), { t: 'openOverlay', overlay: { kind: 'dialog', id: 'x' as never, params: null } });
  expect(escLeavesLauncher(overlay, true, key)).toBe(false);
});
