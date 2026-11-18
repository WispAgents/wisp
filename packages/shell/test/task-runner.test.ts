// QA desktop r3 #1: a follow-up runs on the task's own harness, model and thinking unless the reply box picks another,
// and the screen names that run: never "Default" for a task that ran on Opus.
import { expect, test } from 'vitest';
import { effortWord, runPick, runWords, taskRunner } from '../src/frame/runner.ts';

test('the reply runs on the task’s own model and says so; Default only when it picked none_qa_desktop_r3_1', () => {
  const opus = taskRunner({ agent: 'claude', summary: { model: 'opus', effort: 'high' } });
  expect(opus).toEqual({ agent: 'claude', model: 'opus', effort: 'high' });
  expect(runWords('Claude Code', opus, 'Default')).toBe('Claude Code · opus · High');
  expect(runWords('Claude Code', taskRunner({ agent: 'claude', summary: {} }), 'Default')).toBe('Claude Code · Default');
  expect(taskRunner({ agent: 'codex', summary: { model: 7, effort: null } })).toEqual({ agent: 'codex', model: '', effort: '' });
  expect([effortWord('xhigh'), effortWord('low'), effortWord('')]).toEqual(['X-High', 'Low', '']);
  // nothing picked in the reply box: no `run`, so runtime keeps the task's own (run.rs apply_opts)
  expect(runPick(undefined)).toBeUndefined();
});
