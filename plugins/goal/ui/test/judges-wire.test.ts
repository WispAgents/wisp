// B44: goal.set carries the judges and vote you chose (they were sent nowhere), each judge as the server names one
import { expect, it } from 'vitest';
import { goalDefaults, judgeOf, judgeWire, setGoal, viewJudge } from '../src/commands.ts';
import { harnessOf, liveJudges } from '../src/rules.ts';
import { defaultsOf, judgesOf } from '../src/defaults.ts';

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

// New task's Judges keep the `goal` resource's defaults (v1 defaults.goalJudges / goalVote)
it('the defaults keep the round limit, name judges as the server does, and read back crew by id', () => {
  const have = [{ kind: 'goal', name: 'default', spec: { judges: ['old'], vote: 'any', maxRounds: 7 } }];
  expect(goalDefaults(have, [{ crew: 'c1' }, { agent: 'claude', model: 'opus', effort: 'high' }], 'all')).toEqual({
    kind: 'goal',
    name: 'default',
    spec: { maxRounds: 7, judges: ['c1', 'claude/opus/high'], vote: 'all' },
  });
  expect(goalDefaults([], [], 'majority')).toEqual({ kind: 'goal', name: 'default', spec: { vote: 'majority' } });
  expect(judgeOf('c1', [{ id: 'c1' }])).toEqual({ crew: 'c1' });
  expect(judgeOf('codex', [{ id: 'c1' }])).toEqual({ agent: 'codex', model: '', effort: '' });
  expect(judgeOf('claude/opus/high', [])).toEqual({ agent: 'claude', model: 'opus', effort: 'high' });
});

// B47: a judge's harnesses are wisp.runtime's harness rows (they were read from a `harness` row nobody writes)
it('a harness topic row reads as a judge harness: missing or not available is not available', () => {
  const row = { id: 'claude', ord: 0, state: 'installed' as const, agent: { name: 'Claude Code', available: true, models: [{ id: 'opus', label: 'Opus', hint: 'x' }], efforts: ['high'] } };
  expect(harnessOf(row)).toEqual({ id: 'claude', name: 'Claude Code', available: true, models: [{ id: 'opus', label: 'Opus' }], efforts: ['high'] });
  expect(harnessOf({ ...row, state: 'missing' }).available).toBe(false);
  expect(harnessOf({ ...row, agent: { ...row.agent, name: '' } }).name).toBe('claude');
});

// New task's Judges and Set goal… read the same defaults: the server's `goal` resource
it('the defaults read from the goal resource: names, a vote it knows, else majority; crew told apart by id', () => {
  expect(defaultsOf([{ kind: 'goal', name: 'default', spec: { judges: ['c1', 'codex/gpt-5', 3], vote: 'all' } }])).toEqual({ judges: ['c1', 'codex/gpt-5'], vote: 'all' });
  expect(defaultsOf([{ kind: 'goal', name: 'default', spec: { vote: 'most' } }])).toEqual({ judges: [], vote: 'majority' });
  expect(judgesOf({ judges: ['c1', 'codex/gpt-5', { crew: 'c2' }], vote: 'any' }, [{ id: 'c1', name: 'Ada' } as never])).toEqual([{ crew: 'c1' }, { agent: 'codex', model: 'gpt-5', effort: '' }, { crew: 'c2' }]);
});

// review r328: an existing goal's view names each judge by its wire name; Edit goal… reads them back
it("a goal view's judge reads back as the judge it names; one already split stays", () => {
  const crew = [{ id: 'c1' }];
  expect(viewJudge({ agent: 'claude/opus/high', model: '', effort: '', name: 'claude/opus/high' }, crew)).toEqual({ agent: 'claude', model: 'opus', effort: 'high', name: 'claude/opus/high' });
  expect(viewJudge({ agent: 'c1', model: '', effort: '' }, crew)).toEqual({ crew: 'c1' });
  expect(viewJudge({ agent: 'codex', model: 'gpt-5', effort: '' }, crew)).toEqual({ agent: 'codex', model: 'gpt-5', effort: '' });
});

// B49: a profile's name stays a judge; judges given empty are sent (no judges), not given are left out
it('a bare unknown name (a profile) stays; a gone crew member or harness/model goes; [] is sent, undefined is not', () => {
  const harnesses = [{ id: 'claude', name: 'Claude Code', available: true, models: [] }];
  const live = liveJudges([{ agent: 'reviewer', model: '', effort: '' }, { agent: 'gone', model: 'm', effort: '' }, { crew: 'c9' }, { agent: 'claude', model: 'opus', effort: '' }], [], harnesses);
  expect(live).toEqual([{ agent: 'reviewer', model: '', effort: '' }, { agent: 'claude', model: 'opus', effort: '' }]);
  const wire = (setGoal as unknown as { wire: (a: unknown) => Record<string, unknown> }).wire;
  expect(wire({ session: 'p:t1', goal: { text: 'x', judges: [] }, now: 1 })['judges']).toEqual([]);
  expect('judges' in wire({ session: 'p:t1', goal: { text: 'x' }, now: 1 })).toBe(false);
});
