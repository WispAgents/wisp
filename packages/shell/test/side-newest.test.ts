// UX 2026-11-18 desktop #5: the panel opened on Changes even when it said "No changes yet". It opens on the newest
// thing: the PR once there is one, else Changes when the work changed files, else Shell.
import { expect, test } from 'vitest';
import { newestSide, prBadge } from '../src/frame/side-newest.ts';

test('the_panel_opens_on_the_pr_else_changes_with_a_diff_else_shell', () => {
  expect(newestSide({ summary: { pr: { number: 812 } } }, 0)).toBe('pr');
  expect(newestSide({ summary: { prs: [{ repo: 'a', pr: { number: 3 } }] } }, 0)).toBe('pr');
  expect(newestSide({ summary: { pr: null } }, 4)).toBe('changes');
  expect(newestSide({ summary: { toShip: { dirty: 2 } } }, 0)).toBe('changes');
  expect(newestSide({ summary: { toShip: { dirty: 0, ahead: 1 } } }, 0)).toBe('changes');
  expect(newestSide({ summary: {} }, 0)).toBe('shell');
  expect(newestSide(undefined, 0)).toBe('shell');
});

// QA round 6 phone #14: the phone's PR pill said "Pull request"; it names the PR and its checks, as the desktop's tab
// (UX 2026-11-18 phone #3: `PR #128 ✓`)
test('the_pr_pill_names_the_pr_and_its_checks', () => {
  expect(prBadge({ summary: { pr: { number: 806, checks: { pass: 3, fail: 0, pending: 0 } } } })).toBe('#806 ✓');
  expect(prBadge({ summary: { pr: { number: 806, checks: { pass: 1, fail: 1, pending: 0 } } } })).toBe('#806 ✕');
  expect(prBadge({ summary: { pr: { number: 996, checks: { pass: 0, fail: 0, pending: 0 }, waitingChecks: true } } }), 'checks not listed yet').toBe('#996 ●');
  expect(prBadge({ summary: { pr: { number: 806, checks: { pass: 1, fail: 0, pending: 2 } } } })).toBe('#806 ●');
  expect(prBadge({ summary: { pr: { number: 806 } } })).toBe('#806');
  expect(prBadge({ summary: { prs: [{ repo: 'a', pr: { number: 3 } }, { repo: 'b', pr: { number: 4 } }] } })).toBe('#3');
  expect(prBadge({ summary: { pr: null } })).toBeUndefined();
});
