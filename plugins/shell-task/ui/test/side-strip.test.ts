// The side panel's strip (owner, 2026-11-19, replacing UX 2026-11-18 desktop #5): v1's tabs show by default, labelled,
// whether or not they have something in them yet: Changes, PR, Shell and Preview, then Watch while the task has one and
// the tabs other plugins pin (My Stuff apps, Thread), in v1 panel.js paintName's fixed order, so none ever moves. The
// owner's screenshot had the panel on Shell with one unlabelled tab and the rest hidden behind +.
import { expect, test } from 'vitest';
import { sideStrip, tabCount } from '../src/side/strip.ts';

test('the_strip_shows_changes_pr_shell_and_preview_even_when_they_have_nothing_yet', () => {
  expect(sideStrip({ term: false, watch: false, extras: [] })).toEqual(['changes', 'pr', 'shell', 'preview']);
});

test('a_terminal_task_leads_with_its_cli_watch_comes_after_the_panels_own_and_pinned_tabs_last', () => {
  expect(sideStrip({ term: true, watch: true, extras: ['wisp.stuff|panes|st2', 'wisp.threads|tab|thread'] })).toEqual([
    'term',
    'changes',
    'pr',
    'shell',
    'preview',
    'watch',
    'wisp.stuff|panes|st2',
    'wisp.threads|tab|thread',
  ]);
});

test('the_pr_tab_names_its_number_and_checks', () => {
  expect(tabCount.pr({ number: 812, checks: { pass: 3, fail: 0, pending: 0 } })).toBe('#812 ✓');
  expect(tabCount.pr({ number: 812, checks: { pass: 3, fail: 1, pending: 2 } })).toBe('#812 ✕');
  expect(tabCount.pr({ number: 812, checks: { pass: 3, fail: 0, pending: 2 } })).toBe('#812 ●');
  expect(tabCount.pr({ number: 812 })).toBe('#812');
  expect(tabCount.pr(undefined)).toBeUndefined();
});
