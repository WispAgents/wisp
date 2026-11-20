// Audit H5: New task with a task file (`agent: critic`, codex in its formula) still said Claude Code and Default on the
// runner pill while the file's codex ran. The pill reads what the picked file runs.
import { expect, test } from 'vitest';
import { fileRunner } from '../src/frame/file-runner.ts';

test('a_picked_files_agent_and_model_are_what_the_pill_says_audit_H5', () => {
  const critic = { name: 'a.task.yml', args: { agent: 'critic', formula: { agents: [{ name: 'critic', harness: 'codex', model: 'gpt-5', effort: 'high' }] } } };
  expect(fileRunner(critic)).toEqual({ name: 'critic', harness: 'codex', model: 'gpt-5', effort: 'high' });
  // a harness named directly, its model from the task
  expect(fileRunner({ name: 'b.task.yml', args: { agent: 'codex', model: 'gpt-5' } })).toEqual({ name: 'codex', harness: 'codex', model: 'gpt-5', effort: '' });
  // a formula that names no agent for the task runs the bar's pick: nothing to say
  expect(fileRunner({ name: 'wisp.yml', args: { formula: { agents: [{ name: 'critic', harness: 'codex' }] } } })).toBeUndefined();
  expect(fileRunner(undefined)).toBeUndefined();
});
