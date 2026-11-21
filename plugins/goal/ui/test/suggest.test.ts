// Owner-list #63: `/goal ` in a prompt box suggests the goals declared by name as you type, names that start with the
// word first; a pick puts `/goal <name>` in the box (its value is that text, so the box's Enter sends it as typed in
// full); past a space it's a goal in words and nothing is suggested.
import { expect, test, vi } from 'vitest';
import { goalItems, goalSuggestions, goalWord, MAX } from '../src/suggest.ts';

const goals = [
  { name: 'green-build', text: 'npm test passes' },
  { name: 'docs-clean', text: 'the docs build green' },
  { name: 'Gate', text: 'the release gate holds' },
  { name: 'lint-zero', text: 'eslint reports nothing' },
];

test('slash_goal_suggests_names_starting_with_the_word_then_names_or_descriptions_holding_it', () => {
  expect(goalSuggestions(goals, 'g').map((i) => i.label)).toEqual(['Gate', 'green-build', 'docs-clean', 'lint-zero']);
  expect(goalSuggestions(goals, 'green').map((i) => i.label)).toEqual(['green-build', 'docs-clean']);
  expect(goalSuggestions(goals, '').map((i) => i.label), 'nothing typed yet: every goal, A–Z').toEqual(['docs-clean', 'Gate', 'green-build', 'lint-zero']);
  expect(goalSuggestions(goals, 'zzz')).toEqual([]);
  expect(goalSuggestions(goals, 'green')[0]).toEqual({ value: '/goal green-build', label: 'green-build', hint: 'npm test passes', icon: 'target', insert: '/goal green-build' });
  const many = Array.from({ length: 30 }, (_, i) => ({ name: `g${i}`, text: '' }));
  expect(goalSuggestions(many, 'g')).toHaveLength(MAX);
});

test('only_the_one_word_after_slash_goal_is_suggested_for', () => {
  expect(goalWord('goal gre')).toBe('gre');
  expect(goalWord('goal ')).toBe('');
  expect(goalWord('go')).toBeUndefined();
  expect(goalWord('btw gre')).toBeUndefined();
});

test('the_goals_come_from_the_boxs_peer_asked_once_and_show_once_they_land', async () => {
  const run = vi.fn().mockResolvedValue([{ name: 'green-build', text: 'npm test passes' }, { name: '', text: 'no name' }]);
  const client = { home: 'peerA', run } as never;
  expect(goalItems('goal gr', { client }), 'not here yet').toEqual([]);
  await Promise.resolve();
  await Promise.resolve();
  expect(goalItems('goal gr', { client }).map((i) => i.label)).toEqual(['green-build']);
  expect(goalItems('goal gr', { client, task: 'peerB:t1' })).toEqual([]);
  expect(run.mock.calls.map((c) => (c[1] as { peer: string }).peer)).toEqual(['peerA', 'peerB']);
  expect(goalItems('btw hi', { client })).toEqual([]);
});

test('a name typed whole shows no menu, so the first Enter sends it_audit_G14', () => {
  const goals = [
    { name: 'green-build', text: 'npm test passes' },
    { name: 'green-build-fast', text: 'quick' },
  ];
  expect(goalSuggestions(goals, 'green-build')).toEqual([]);
  expect(goalSuggestions(goals, 'GREEN-BUILD')).toEqual([]);
  expect(goalSuggestions(goals, 'green').map((i) => i.label)).toEqual(['green-build', 'green-build-fast']);
});
