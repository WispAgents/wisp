// UX 2026-11-18 desktop #5: the panel opened on Changes even when it said "No changes yet". It opens on the newest
// thing: the PR once there is one, else Changes when the work changed files, else Shell.
import { expect, test } from 'vitest';
import { newestSide } from '../src/frame/side-newest.ts';

test('the_panel_opens_on_the_pr_else_changes_with_a_diff_else_shell', () => {
  expect(newestSide({ summary: { pr: { number: 812 } } }, 0)).toBe('pr');
  expect(newestSide({ summary: { prs: [{ repo: 'a', pr: { number: 3 } }] } }, 0)).toBe('pr');
  expect(newestSide({ summary: { pr: null } }, 4)).toBe('changes');
  expect(newestSide({ summary: { toShip: { dirty: 2 } } }, 0)).toBe('changes');
  expect(newestSide({ summary: { toShip: { dirty: 0, ahead: 1 } } }, 0)).toBe('changes');
  expect(newestSide({ summary: {} }, 0)).toBe('shell');
  expect(newestSide(undefined, 0)).toBe('shell');
});
