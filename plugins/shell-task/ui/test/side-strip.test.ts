// UX 2026-11-18 desktop #5: the side panel showed all five tabs (Changes, PR, Shell, Preview, Thread) whether or not
// they had anything. The strip holds the tabs with something in them (and the one open); the rest wait behind +. A tab
// that joined the strip stays there (it never jumps out when its content goes), in the panel's own order.
import { expect, test } from 'vitest';
import { sideStrip, tabCount } from '../src/side/strip.ts';

const order = ['changes', 'pr', 'shell', 'preview'];

test('only_tabs_with_something_show_the_rest_wait_behind_plus', () => {
  const r = sideStrip({ order, has: new Set(['shell']), current: 'shell', joined: [] });
  expect(r.strip).toEqual(['shell']);
  expect(r.more).toEqual(['changes', 'pr', 'preview']);
  // a diff and a PR came: they join in the panel's order
  expect(sideStrip({ order, has: new Set(['changes', 'pr', 'shell']), current: 'shell', joined: r.strip }).strip).toEqual(['changes', 'pr', 'shell']);
});

test('the_open_tab_is_always_in_the_strip_and_a_tab_once_shown_stays', () => {
  expect(sideStrip({ order, has: new Set(['shell']), current: 'preview', joined: [] }).strip).toEqual(['shell', 'preview']);
  // the diff was committed away: Changes keeps its place
  expect(sideStrip({ order, has: new Set(['shell']), current: 'shell', joined: ['changes', 'shell'] })).toEqual({ strip: ['changes', 'shell'], more: ['pr', 'preview'] });
  // a tab gone from the panel (a pin removed) leaves
  expect(sideStrip({ order: ['shell'], has: new Set(['shell']), current: 'shell', joined: ['changes', 'shell', 'pin'] }).strip).toEqual(['shell']);
});

test('the_pr_tab_names_its_number_and_checks', () => {
  expect(tabCount.pr({ number: 812, checks: { pass: 3, fail: 0, pending: 0 } })).toBe('#812 ✓');
  expect(tabCount.pr({ number: 812, checks: { pass: 3, fail: 1, pending: 2 } })).toBe('#812 ✕');
  expect(tabCount.pr({ number: 812, checks: { pass: 3, fail: 0, pending: 2 } })).toBe('#812 ●');
  expect(tabCount.pr({ number: 812 })).toBe('#812');
  expect(tabCount.pr(undefined)).toBeUndefined();
});
