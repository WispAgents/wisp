import { expect, test } from 'vitest';
import { copiesOf, MAX_COPIES, nextAgent, teamFromSetting, teamSize, teamToSend, uniqueNames, type Delegate } from '../src/frame/team.ts';

test('new_task_prefills_its_team_from_settings_and_sends_each_delegate_as_settings_names_it', () => {
  const setting = [{ name: 'reviewer', agent: 'codex', model: 'gpt-5', use: 'review' }, { bad: true }, { name: 'x', agent: '' }];
  expect(teamFromSetting(setting)).toEqual([{ name: 'reviewer', agent: 'codex', model: 'gpt-5', use: 'review' }]);
  expect(teamFromSetting(null)).toEqual([]);
  expect(teamToSend([...teamFromSetting(setting), { name: '', agent: 'grok' }])).toEqual([
    { name: 'reviewer', agent: 'codex', model: 'gpt-5', use: 'review' },
    { name: 'grok', agent: 'grok' },
  ]);
});

test('picking many delegates sends them all: no cap at six_v1_no_team_cap', () => {
  const picked: Delegate[] = ['claude', 'codex', 'opencode', 'cursor', 'copilot', 'grok', 'antigravity', 'gemini'].map((agent) => ({ name: '', agent }));
  expect(teamToSend(picked)).toHaveLength(8);
});

test('delegate_names_follow_v1_blank_is_the_harness_repeats_get_a_number_v1_team_unique_names', () => {
  expect(uniqueNames([{ name: '', agent: 'claude' }, { name: '', agent: 'claude' }, { name: 'My Tester!', agent: 'codex' }, { name: 'my tester', agent: 'grok' }]).map((m) => m.name)).toEqual([
    'claude',
    'claude-2',
    'my-tester',
    'my-tester-2',
  ]);
});

test('copies_count_in_the_team_size_and_are_sent_from_two_up_to_five_v1_tp_copies', () => {
  const team: Delegate[] = [
    { name: '', agent: 'codex', count: 3 },
    { name: '', agent: 'grok', count: 99 },
    { name: '', agent: 'cursor' },
  ];
  expect(copiesOf(team[1] as Delegate)).toBe(MAX_COPIES);
  expect(teamSize(team)).toBe(3 + MAX_COPIES + 1);
  const sent = teamToSend(team);
  expect(sent[0]).toEqual({ name: 'codex', agent: 'codex', count: 3 });
  expect(sent[2]).toEqual({ name: 'cursor', agent: 'cursor' });
});

test('a_delegate_away_from_the_lead_is_sent_with_its_machine_and_its_use_trimmed', () => {
  expect(teamToSend([{ name: 'gpu', agent: 'opencode', machine: 'peer-1', use: '  heavy lifting ', modelLabel: 'Qwen' }])).toEqual([{ name: 'gpu', agent: 'opencode', machine: 'peer-1', use: 'heavy lifting' }]);
});

test('add_delegate_picks_a_harness_not_on_the_team_nor_the_lead_v1_team_add', () => {
  const ids = ['claude', 'codex', 'opencode'];
  expect(nextAgent('claude', [], ids)).toBe('codex');
  expect(nextAgent('claude', [{ name: '', agent: 'codex' }], ids)).toBe('opencode');
  // all taken: any but the lead, then the lead itself when it's the only one
  expect(nextAgent('claude', [{ name: '', agent: 'codex' }, { name: '', agent: 'opencode' }], ids)).toBe('codex');
  expect(nextAgent('claude', [], ['claude'])).toBe('claude');
  expect(nextAgent('claude', [], [])).toBeUndefined();
});
