// Rerun… (src/frame/rerun.ts, v1 actions.js rerunDialog): a new task from the old one's spec, as edited, where picked.
import { expect, test } from 'vitest';
import { rerunArgs, rerunPlace, startEdits } from '../src/frame/rerun.ts';

const spec = { prompt: 'Fix the login bug', steps: ['then run the tests'], agent: 'claude', model: 'opus', effort: 'high', mode: 'edits', cwd: '/code/web', dirs: ['/code/api'] };

test('the dialog starts as the task ran; the prompt, mode and harness can change; the old task is kept_v1_rerun_dialog', () => {
  const e = startEdits(spec, 'laptop');
  expect(e).toEqual({ prompt: 'Fix the login bug', mode: 'edits', agent: 'claude', model: 'opus', effort: 'high', peer: 'laptop' });
  expect(rerunArgs(spec, { ...e, prompt: '  Fix the login bug, and the signup one  ', mode: 'plan' }, { of: 'laptop:s1', id: 'laptop:s2', at: 5 })).toEqual({
    peer: 'laptop',
    id: 'laptop:s2',
    prompt: 'Fix the login bug, and the signup one',
    agent: 'claude',
    mode: 'plan',
    at: 5,
    model: 'opus',
    effort: 'high',
    cwd: '/code/web',
    dirs: ['/code/api'],
    steps: ['then run the tests'],
    lineage: { rerunOf: 's1' },
  });
  // a harness picked from the toolbar's menu starts on its own default model
  expect(startEdits(spec, 'laptop', 'codex')).toMatchObject({ agent: 'codex', model: '', effort: '' });
});

test('picked on another computer, it runs there_v1_rerun_elsewhere', () => {
  const e = { ...startEdits(spec, 'laptop'), peer: 'studio' };
  expect(rerunArgs(spec, e, { of: 'laptop:s1', id: 'studio:s9', at: 1 })).toMatchObject({ peer: 'studio', id: 'studio:s9', lineage: { rerunOf: 'laptop:s1' } });
});

test('a rerun is the same kind of task: a My Stuff pane stays a pane on another harness (owner, 2026-11-17)', () => {
  const args = rerunArgs({ prompt: 'a hello world pane', agent: 'claude', ext: { 'wisp.stuff': { kind: 'pane' } } }, { prompt: 'a hello world pane', mode: 'auto', agent: 'codex', model: '', effort: '', peer: 'local' }, { of: 'local:t1', id: 'local:t2', at: 1 });
  expect(args).toMatchObject({ agent: 'codex', ext: { 'wisp.stuff': { kind: 'pane' } } });
  expect('ext' in rerunArgs({ prompt: 'x' }, { prompt: 'x', mode: 'auto', agent: '', model: '', effort: '', peer: 'local' }, { of: 'a', id: 'b', at: 1 })).toBe(false);
});

// QA round 15 phone #4: a task started on Auto keeps `auto` in its spec, and the Rerun dialog's harness chip read
// "auto" though the task ran on Claude Code and Auto is no longer offered: it starts on the harness it ran on
test('a_rerun_of_an_auto_task_starts_on_the_harness_it_ran_on_qa_r15_phone_4', () => {
  const ran = { agent: 'claude', model: 'opus', effort: 'high' };
  expect(startEdits({ prompt: 'p', agent: 'auto', mode: 'auto' }, 'laptop', undefined, ran)).toMatchObject({ agent: 'claude', model: 'opus', effort: 'high' });
  expect(startEdits({ prompt: 'p', mode: 'auto' }, 'laptop', undefined, ran)).toMatchObject({ agent: 'claude' });
  // a spec naming its harness keeps it; a pick from the toolbar's menu still wins
  expect(startEdits({ prompt: 'p', agent: 'codex', model: 'gpt-5' }, 'laptop', undefined, ran)).toMatchObject({ agent: 'codex', model: 'gpt-5' });
  expect(startEdits({ prompt: 'p', agent: 'auto' }, 'laptop', 'cursor', ran)).toMatchObject({ agent: 'cursor', model: '', effort: '' });
});

// audit B #2: Rerun… of a task that ran in a worktree ran its copy in the main checkout: the args had no `worktree`, and
// `rerunOf` was the entity key (`laptop:s1`), which the peer's create (its own-worktree rule for reruns) matched to no task
test('a_rerun_of_a_worktree_task_runs_in_a_worktree_of_its_own_audit_b_2', () => {
  const e = startEdits(spec, 'laptop');
  const args = rerunArgs({ ...spec, worktree: true }, e, { of: 'laptop:s1', id: 'laptop:s2', at: 1 });
  expect(args).toMatchObject({ cwd: '/code/web', worktree: true, lineage: { rerunOf: 's1' } });
  // in place stays in place; a spec that doesn't say leaves it to the peer
  expect(rerunArgs({ ...spec, worktree: false }, e, { of: 'laptop:s1', id: 'laptop:s2', at: 1 })).toMatchObject({ worktree: false });
  expect('worktree' in rerunArgs(spec, e, { of: 'laptop:s1', id: 'laptop:s2', at: 1 })).toBe(false);
});

// Audit C #6: the dialog said "Starts a new task in /tmp/proj" (the main checkout) while the rerun of a worktree task
// gets a new worktree of its own: it says where the rerun really runs.
test('the hint says a worktree task reruns in a new worktree of its project, not in the main checkout_audit_c_6', () => {
  expect(rerunPlace({ prompt: 'x', cwd: '/private/tmp/qaC1/proj', worktree: true })).toEqual({ key: 'shell.rerun.hintWorktree', vars: { project: 'proj' } });
  expect(rerunPlace({ prompt: 'x', cwd: 'C:\\code\\web\\', worktree: true })).toEqual({ key: 'shell.rerun.hintWorktree', vars: { project: 'web' } });
  expect(rerunPlace({ prompt: 'x', cwd: '/code/web', worktree: false })).toEqual({ key: 'shell.rerun.hint', vars: { cwd: '/code/web' } });
  expect(rerunPlace({ prompt: 'x', cwd: '/code/web' })).toEqual({ key: 'shell.rerun.hint', vars: { cwd: '/code/web' } });
  // a task with no folder: no empty "in  on" (seen in the dev app's Rerun on a plugin task)
  expect(rerunPlace({ prompt: 'x' })).toEqual({ key: 'shell.rerun.hintNoFolder', vars: {} });
  expect(rerunPlace({ prompt: 'x', cwd: '', worktree: true })).toEqual({ key: 'shell.rerun.hintNoFolder', vars: {} });
});
