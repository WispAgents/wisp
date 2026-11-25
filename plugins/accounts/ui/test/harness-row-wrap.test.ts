// QA phone r3 #1: on a 360 phone Claude Code's controls (Signed in, Sign out, Update to 2.1.327, Ready) didn't fit one
// line, and "Update to …" sat over "Sign out": they were an Inline, which never wraps. They're separate actions, so a
// Wrap: one line where they fit (desktop), more lines where they don't (design/layout.md: only Wrap groups wrap).
import { expect, test } from 'vitest';
// the row is the picked harness's Account & auth (HarnessDetail.tsx) since Settings → Agents became a list and its detail
import PANE from '../src/panes/HarnessDetail.tsx?raw';

test('a_harness_rows_controls_are_a_wrap_group', () => {
  const row = PANE.indexOf('part="accounts.harness"');
  const control = PANE.indexOf('control={', row);
  const group = PANE.slice(control, control + 80);
  expect(group).toMatch(/<Wrap\b/);
});
