// Audit H5: New task with a task file (`agent: critic`, codex in its formula) still said Claude Code and Default on the
// runner pill while the file's codex ran. The pill reads what the picked file runs.
import { expect, test } from 'vitest';
import { barArgs, fileRunner } from '../src/frame/file-runner.ts';

test('a_picked_files_agent_and_model_are_what_the_pill_says_audit_H5', () => {
  const critic = { name: 'a.task.yml', args: { agent: 'critic', formula: { agents: [{ name: 'critic', harness: 'codex', model: 'gpt-5', effort: 'high' }] } } };
  expect(fileRunner(critic)).toEqual({ name: 'critic', harness: 'codex', model: 'gpt-5', effort: 'high' });
  // a harness named directly, its model from the task
  expect(fileRunner({ name: 'b.task.yml', args: { agent: 'codex', model: 'gpt-5' } })).toEqual({ name: 'codex', harness: 'codex', model: 'gpt-5', effort: '' });
  // a formula that names no agent for the task runs the bar's pick: nothing to say
  expect(fileRunner({ name: 'wisp.yml', args: { formula: { agents: [{ name: 'critic', harness: 'codex' }] } } })).toBeUndefined();
  expect(fileRunner(undefined)).toBeUndefined();
});

// Audit I4: with the bar on Claude Code · Opus and a task file picked (`agent: critic`, codex gpt-5 high in its formula),
// `runtime.create` carried `"model":"opus"` beside the file's agent, and codex ran `-m opus`. The bar's model, thinking
// and account go only when no picked file says what runs.
test('a_picked_task_files_agent_runs_its_own_model_never_the_bars_audit_I4', () => {
  const critic = { name: 'a.task.yml', args: { agent: 'critic', formula: { agents: [{ name: 'critic', harness: 'codex', model: 'gpt-5', effort: 'high' }] } } };
  const bar = { agent: 'claude', model: 'opus', effort: 'low', account: 'main' };
  expect(barArgs(fileRunner(critic), bar)).toEqual({});
  // the body New task sends: the bar's picks, then the file's own args over them
  // the bar's agent first (as New task's `runner`), then the bar's picks, then the file's own args over them
  const runner: Record<string, unknown> = { agent: bar.agent };
  const body: Record<string, unknown> = { prompt: 'x', ...runner, ...barArgs(fileRunner(critic), bar), ...critic.args };
  expect(body).not.toHaveProperty('model');
  expect(body).not.toHaveProperty('effort');
  expect(body['agent']).toBe('critic');
  // no file, or one that names nothing to run: the bar's picks go as before; Auto sends none
  expect(barArgs(undefined, bar)).toEqual({ model: 'opus', effort: 'low', account: 'main' });
  expect(barArgs(fileRunner({ name: 'wisp.yml', args: { formula: { agents: [] } } }), bar)).toEqual({ model: 'opus', effort: 'low', account: 'main' });
  expect(barArgs(undefined, { agent: 'auto', model: 'opus' })).toEqual({});
  expect(barArgs(undefined, { agent: 'claude' })).toEqual({});
});
