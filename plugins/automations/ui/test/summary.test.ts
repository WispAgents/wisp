import { expect, test } from 'vitest';
import { lastRunWords, recipeNames, summary } from '../src/panes/summary.ts';

test('automations_lines_say_none_yet_or_name_what_there_is_as_v1_did', () => {
  expect(summary([], 'save a prompt you run often')).toBe('None yet · save a prompt you run often');
  expect(summary(['Triage', 'Deps'], 'x')).toBe('Triage, Deps');
  expect(summary(['a', 'b', 'c', 'd', 'e'], 'x')).toBe('a, b, c and 2 more');
});

// Audit B #16: just after a run the trigger row read "last run now ago".
test('a_trigger_run_a_moment_ago_reads_last_run_just_now', () => {
  const words: Record<string, string> = { 'automations.page.notRun': 'not run yet', 'automations.page.lastRunNow': 'last run just now', 'automations.page.lastRun': 'last run {ago} ago' };
  const say = (k: string, v?: Readonly<Record<string, string>>): string => (words[k] ?? k).replace('{ago}', v?.['ago'] ?? '');
  const ago = (at: number): string => `${Math.round((10_000_000 - at) / 60_000)}m`;
  expect(lastRunWords(10_000_000 - 5_000, 10_000_000, ago, say)).toBe('last run just now');
  expect(lastRunWords(10_000_000 - 600_000, 10_000_000, ago, say)).toBe('last run 10m ago');
  expect(lastRunWords(null, 10_000_000, ago, say)).toBe('not run yet');
});

// Audit E4 (D11's last part): a v1 recipe kept its name as written in `title` ("Run tests"); the Recipes line named it by
// its key (`run-tests`), while the launcher's chip and Manage read "Run tests"
test('the_recipes_line_names_a_recipe_as_written_audit_e4', () => {
  expect(recipeNames([{ name: 'run-tests', title: 'Run tests' }, { name: 'triage' }, { name: 'x', title: '  ' }])).toEqual(['Run tests', 'triage', 'x']);
});
