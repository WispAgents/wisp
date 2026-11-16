// B44: goal.set carries the judges and vote you chose (they were sent nowhere), each judge as the server names one
import { expect, it } from 'vitest';
import { judgeWire, setGoal } from '../src/commands.ts';

it('a crew member by name, a harness as harness/model/effort without empty trailing parts', () => {
  expect(judgeWire({ crew: 'Ada' })).toBe('Ada');
  expect(judgeWire({ agent: 'codex', model: 'gpt-5', effort: 'high' })).toBe('codex/gpt-5/high');
  expect(judgeWire({ agent: 'claude', model: '', effort: '' })).toBe('claude');
});

it('goal.set sends the text, the judges and the vote', () => {
  const wire = (setGoal as unknown as { wire: (a: unknown) => unknown }).wire;
  expect(wire({ session: 'p:t1', goal: { text: 'tests pass', judges: [{ crew: 'Ada' }, { agent: 'codex', model: 'gpt-5', effort: '' }], vote: 'all' }, now: 1 })).toEqual({
    id: 't1',
    text: 'tests pass',
    judges: ['Ada', 'codex/gpt-5'],
    vote: 'all',
  });
});
