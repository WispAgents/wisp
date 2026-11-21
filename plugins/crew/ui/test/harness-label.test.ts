// A member with no harness of its own (`agent: auto`) reads as New task while Auto is off (owner, 2026-11-17), on the
// crew page, the phone and the import review (audit A #22: "Auto · Full auto", "Every 30 min · auto").
import { expect, test } from 'vitest';
import { harnessLabel } from '../src/harness-label.ts';
import '../src/i18n/lazy.ts';

const list = [{ id: 'claude', name: 'Claude Code', logo: 'c.svg' }];

test('auto_reads_as_new_task_while_auto_is_off_and_a_harness_as_its_name_audit_A_22', () => {
  expect(harnessLabel('auto', list, false).name).toBe('As New task');
  expect(harnessLabel('', list, false).name).toBe('As New task');
  expect(harnessLabel('auto', list, true).name).toBe('Auto');
  expect(harnessLabel('claude', list, false)).toEqual({ name: 'Claude Code', logo: 'c.svg' });
  expect(harnessLabel('grok', list, false).name).toBe('grok');
});
