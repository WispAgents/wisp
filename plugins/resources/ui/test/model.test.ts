import { expect, test } from 'vitest';
import { changeLines, formatOf, groups, kindCounts, picked, planShown, sectionTitle, type Plan } from '../src/model.ts';

test('setup_import_reads_the_plan_as_cards_and_applies_only_whats_ticked', () => {
  expect([formatOf('wisp: 1\n'), formatOf(' {"wisp":1}'), formatOf('wisp = 1\n')]).toEqual(['yaml', 'json', 'toml']);
  const kinds = [
    { kind: 'agent', section: 'agents', scope: 'shared', order: 2 },
    { kind: 'crew', section: 'crew', scope: 'shared', order: 1 },
    { kind: 'account', section: 'accounts', scope: 'personal', order: 0 },
  ];
  const have = [{ kind: 'crew', name: 'ada', spec: {} }, { kind: 'agent', name: 'fast', spec: {} }, { kind: 'agent', name: 'deep', spec: {} }, { kind: 'account', name: 'me', spec: {} }];
  expect(kindCounts(kinds, have)).toEqual([
    { kind: 'crew', section: 'crew', count: 1 },
    { kind: 'agent', section: 'agents', count: 2 },
  ]);
  const plan: Plan = {
    unchanged: 1,
    ops: [
      { op: 'create', kind: 'crew', name: 'bob', spec: {} },
      { op: 'update', kind: 'agent', name: 'fast', spec: {}, changes: [{ path: '/model', from: 'sonnet', to: 'opus' }] },
      { op: 'create', kind: 'trigger', name: 'ci', spec: {} },
    ],
  };
  expect(groups(plan).map((g) => [g.id, g.ops.length])).toEqual([
    ['create', 2],
    ['update', 1],
  ]);
  expect(changeLines(plan.ops[1]!)).toEqual(['model: sonnet → opus']);
  // audit D6: a setting's whole value has no field name: the values alone, no bare colon
  expect(changeLines({ op: 'update', kind: 'setting', name: 'settings.editor', spec: 'cursor', changes: [{ path: '', from: 'zed', to: 'cursor' }] })).toEqual(['zed → cursor']);
  const pasted = [{ kind: 'crew', name: 'bob', spec: {} }, { kind: 'agent', name: 'fast', spec: {} }, { kind: 'trigger', name: 'ci', spec: {} }, { kind: 'agent', name: 'same', spec: {} }];
  expect(picked(pasted, plan, new Set(['trigger/ci'])).map((r) => r.name)).toEqual(['bob', 'fast']);
});

test('setup_names_each_kind_in_words_not_its_raw_id_qa_r1_28', () => {
  expect(['plugins', 'projects', 'router', 'goal', 'channels', 'notifications', 'crew', 'mcp-servers'].map(sectionTitle)).toEqual([
    'Plugins',
    'Projects',
    'Router',
    'Goal',
    'Channels',
    'Notifications',
    'Crew',
    'Mcp servers',
  ]);
});

// audit C8: after a plan for one text, pasting another that fails left the first plan under the error ("It can't be
// applied · settings.worktrees.max … not 33"). A plan shows only while the box holds the text it was made from.
test('a_plan_shows_only_for_the_text_it_was_made_from', () => {
  const p = { ops: [], unchanged: 0 } as unknown as Plan;
  const made = { text: 'wisp: 1\nsettings:\n  worktrees.max: 33', plan: p };
  expect(planShown(made, made.text)).toBe(p);
  expect(planShown(made, 'hello')).toBeUndefined(); // edited (or a later look failed: nothing made for it)
  expect(planShown(made, `${made.text}\n`)).toBeUndefined();
  expect(planShown(undefined, made.text)).toBeUndefined();
});
