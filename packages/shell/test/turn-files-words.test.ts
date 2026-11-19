// Audit F orchestration F4: a turn that changed 2 files said "2 file +2 −0" ('{n} file', no plural forms).
import { expect, test } from 'vitest';
import { SHELL_STRINGS as en } from '../src/i18n/en.ts';

test('a_turns_foot_says_1_file_and_2_files_audit_f_orchestration_f4', () => {
  const t = (k: string, n: number): string => String((en as Record<string, string>)[k]).replace('{count}', String(n));
  expect(t('shell.task.files.one', 1)).toBe('1 file');
  expect(t('shell.task.files.other', 2)).toBe('2 files');
  expect((en as Record<string, string>)['shell.task.files']).toBeUndefined();
});
