// Audit G13: a factory run's Stop the run stopped every station at once, with no question; Delete task didn't say its
// stations' tasks (and their clean worktrees) go with it.
import { expect, test } from 'vitest';
import { SHELL_STRINGS } from '../src/i18n/en.ts';
import '../src/i18n/lazy.ts';
import { deleteWords } from '../src/frame/delete-words.ts';
import { factoryStopQuestion } from '../src/frame/stop-question.ts';

const cat = SHELL_STRINGS as Readonly<Record<string, string>>;
const englishT = (k: string): string => cat[k] ?? k;

test('a factory run asks before it stops, and its delete says the stations go with it_audit_G13', () => {
  const q = factoryStopQuestion(englishT);
  expect(q).toMatchObject({ title: 'Stop the factory run?', ok: 'Stop the run', factory: true });
  expect(q.body).toContain('Every station still working stops');
  expect(deleteWords({ factory: { name: 'strict' } })).toBe('shell.delete.bodyFactory');
  expect(deleteWords({})).toBe('shell.delete.body');
  expect(englishT('shell.delete.bodyFactory')).toContain('stations’ tasks go with it');
});
