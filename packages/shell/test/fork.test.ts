// Fork… (v1 actions.js forkTask; QA round 14 desktop #1): the dialog's start, the turn it forks from, and the two
// commands its confirm sends (nothing before it).
import { expect, test } from 'vitest';
import { forkSteps, forkTitle, forkTurn, startPick } from '../src/frame/fork.ts';

test('the_fork_starts_on_the_tasks_harness_when_its_machine_has_it_else_the_first_it_has', () => {
  expect(startPick({ agent: 'claude', model: 'opus', effort: 'high', mode: 'plan' }, ['codex', 'claude'])).toEqual({ prompt: '', mode: 'plan', agent: 'claude', model: 'opus', effort: 'high' });
  // its machine hasn't the task's harness: the first it has, on its own model and thinking
  expect(startPick({ agent: 'claude', model: 'opus', effort: 'high' }, ['codex'])).toEqual({ prompt: '', mode: 'auto', agent: 'codex', model: '', effort: '' });
});

test('it_forks_from_the_turn_asked_else_the_latest_finished_never_a_running_one', () => {
  const turns = [{ id: 'a', status: 'done' }, { id: 'b', status: 'done' }, { id: 'c', status: 'running' }];
  expect(forkTurn(turns)?.id).toBe('b');
  expect(forkTurn(turns, 'a')?.id).toBe('a');
  expect(forkTurn(turns, 'c')?.id).toBe('b');
  expect(forkTurn([{ id: 'x', status: 'running' }])).toBeUndefined();
  expect(forkTurn([{ id: 'a', status: 'done' }, { id: 's', status: 'starting' }], 's')?.id).toBe('a');
});

test('its_confirm_sends_the_copy_then_what_to_try_on_the_harness_and_mode_picked', () => {
  const s = forkSteps({ prompt: '  Try a token bucket ', mode: 'edits', agent: 'codex', model: 'gpt-5', effort: '' }, { of: 'p:1', newId: 'p:2', turnId: 't1', firstTurnId: 'p:t9', idx: 3, at: 5 });
  expect(s.fork).toEqual({ id: 'p:1', newId: 'p:2', at: 5, title: 'Try a token bucket', agent: 'codex', turnId: 't1' });
  expect(s.followUp).toEqual({ id: 'p:2', text: 'Try a token bucket', turnId: 'p:t9', idx: 3, at: 5, mode: 'edits', run: { agent: 'codex', model: 'gpt-5' } });
});

test('the_fork_is_named_after_what_it_should_try_cut_at_a_word', () => {
  expect(forkTitle('Use a token bucket\nand more')).toBe('Use a token bucket');
  const long = 'Rewrite the rate limiter with a sliding window and keep the old one behind a flag';
  expect(forkTitle(long)).toBe('Rewrite the rate limiter with a sliding window and keep the…');
});
