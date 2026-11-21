import { expect, test } from 'vitest';
import { builtinRows, builtinSteps, delegateRows, hasSubagents, relative, type StepRowLike, type TaskRowLike } from '../src/subagents.ts';
import type { Say } from '../src/team.ts';

const say: Say = (k) => k;
const names: Record<string, string> = { claude: 'Claude Code', codex: 'Codex' };
const harnessName = (id: string): string => names[id] ?? id;

let seq = 0;
const step = (kind: string, t: number, body: Record<string, unknown>, session = 'lead1'): StepRowLike => ({ session_id: session, seq: seq++, t, kind, body: { kind, ...body } });

test('wisps_own_delegates_are_rows_with_harness_and_model_and_crew_jobs_are_left_to_the_crews_rows', () => {
  const tasks: TaskRowLike[] = [
    { id: 'd2', parent_id: 'lead1', title: 'Write the docs', state: 'done', agent: 'claude', created_at: 20, summary: { model: 'opus', preview: 'Docs written', startedAt: 20, endedAt: 80 } },
    { id: 'd1', parent_id: 'lead1', title: 'Fix the test', state: 'running', agent: 'codex', created_at: 10, summary: { model: 'gpt-5', live: 'Running cargo test', startedAt: 10 } },
    { id: 'j1', parent_id: 'lead1', title: 'A crew job', state: 'done', agent: 'claude', body: { ext: { 'wisp.crew': { id: 'ada' } } } },
    { id: 'x1', parent_id: 'other', title: 'Not ours', state: 'done', agent: 'claude' },
  ];
  // the lead's key is `peer:id`; a delegate names it by its bare id
  const rows = delegateRows(tasks, 'me:lead1', harnessName, say);
  expect(rows.map((r) => r.id)).toEqual(['d1', 'd2']);
  expect(rows[0]).toMatchObject({ kind: 'delegate', agent: 'codex', handle: 'Fix the test', line: 'Codex · gpt-5 — Running cargo test', live: true, running: true, startedAt: 10 });
  expect(rows[1]).toMatchObject({ handle: 'Write the docs', line: 'Claude Code · opus — Docs written', status: 'wisp.crew.team.done', running: false, endedAt: 80 });
});

test('a_harnesss_own_subagent_shows_its_current_step_then_its_answer', () => {
  seq = 0;
  const steps = [
    step('tool', 100, { tool: 'agent', label: 'Delegating: Find the flaky test', callId: 'a1' }),
    step('tool', 110, { tool: 'read', label: 'Reading src/login.ts', parent: 'a1' }),
    step('tool', 120, { tool: 'run', label: 'Running npm test', parent: 'a1' }),
    step('tool', 130, { tool: 'agent', label: 'Delegating: Check the docs', callId: 'a2' }),
    step('tool', 140, { tool: 'read', label: 'Reading README.md' }), // the lead's own step
    step('tool_output', 150, { callId: 'a2', text: 'The docs are fine.\nNothing to change.' }),
  ];
  const rows = builtinRows(steps, 'me:lead1', 'claude', 'opus', true, harnessName, say);
  expect(rows.map((r) => r.handle)).toEqual(['Find the flaky test', 'Check the docs']);
  expect(rows[0]).toMatchObject({ kind: 'builtin', call: 'a1', line: 'Claude Code · opus — Running npm test', live: true, running: true, startedAt: 100 });
  expect(rows[0]?.endedAt).toBeUndefined();
  expect(rows[1]).toMatchObject({ line: 'Claude Code · opus — The docs are fine.', running: false, status: 'wisp.crew.team.done', endedAt: 150 });
  // the lead stopped with the first still out: it was cut off with it, not working
  expect(builtinRows(steps, 'me:lead1', 'claude', 'opus', false, harnessName, say)[0]).toMatchObject({ running: false, status: 'wisp.crew.team.stopped' });
  expect(builtinSteps(steps, 'me:lead1', 'a1')).toEqual({ steps: ['Reading src/login.ts', 'Running npm test'], answer: '' });
  expect(builtinSteps(steps, 'me:lead1', 'a2').answer).toBe('The docs are fine.\nNothing to change.');
});

test('the_team_tab_shows_for_a_task_with_subagents_of_its_own', () => {
  seq = 0;
  expect(hasSubagents([], [], 'me:lead1')).toBe(false);
  expect(hasSubagents([{ id: 'd1', parent_id: 'lead1' }], [], 'me:lead1')).toBe(true);
  expect(hasSubagents([{ id: 'j1', parent_id: 'lead1', body: { ext: { 'wisp.crew': { id: 'ada' } } } }], [], 'me:lead1')).toBe(false);
  expect(hasSubagents([], [step('tool', 1, { tool: 'agent', label: 'Delegating: x', callId: 'a1' })], 'me:lead1')).toBe(true);
  expect(hasSubagents([], [step('tool', 1, { tool: 'agent', callId: 'a1' }, 'other')], 'me:lead1')).toBe(false);
});

test('a_subagents_line_names_files_from_the_tasks_folder', () => {
  expect(relative('The first line of `/Users/me/.wisp/worktrees/app-1/README.md` is:', '/Users/me/.wisp/worktrees/app-1')).toBe('The first line of `README.md` is:');
  expect(relative('Reading /private/tmp/w/src/a.ts', '/tmp/w')).toBe('Reading src/a.ts');
  expect(relative('No paths here', '')).toBe('No paths here');
});
