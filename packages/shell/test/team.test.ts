import { expect, test } from 'vitest';
import { copiesOf, deletePreset, MAX_COPIES, nextAgent, presetsOf, savePreset, teamFromSetting, teamSize, teamToSend, uniqueNames, type Delegate } from '../src/frame/team.ts';
import { followUp } from '../src/frame/commands.ts';

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

test('the_reply_box_team_goes_with_the_follow_up_and_a_tasks_kept_team_reads_back_f_comp_33', () => {
  const team = teamToSend([{ name: '', agent: 'codex', count: 2 }]);
  const wire = followUp.wire?.({ id: 'home:01a1e9e05eb676c799c41712ad5656b7', text: 'go', turnId: 'home:01a1e9e05eb676c799c41712ad5656b8', idx: 1, at: 1, team }) as Record<string, unknown>;
  expect(wire['team']).toEqual([{ name: 'codex', agent: 'codex', count: 2 }]);
  // [] goes too: it takes the team away
  expect((followUp.wire?.({ id: 'home:a', text: 'go', turnId: 'home:b', idx: 1, at: 1, team: [] }) as Record<string, unknown>)['team']).toEqual([]);
  // the roster as the runtime keeps it (normalize_team) reads back as the panel's rows, a crew member as it came
  const kept = [
    { name: 'codex', agent: 'codex', model: '', effort: '', use: '', count: 2 },
    { name: 'ada', crew: 'ada01abc', label: 'Ada', role: 'Reviews', agent: 'auto', model: '', effort: '', use: 'review', count: 1 },
  ];
  expect(teamFromSetting(kept)).toEqual([
    { name: 'codex', agent: 'codex', count: 2 },
    { name: 'ada', agent: 'auto', use: 'review', crew: 'ada01abc', label: 'Ada', role: 'Reviews' },
  ]);
  expect(teamToSend(teamFromSetting(kept))[1]).toEqual({ name: 'ada', agent: 'auto', use: 'review', crew: 'ada01abc', label: 'Ada', role: 'Reviews' });
});

test('presets_read_from_the_setting_save_by_name_and_delete_v1_config_teams', () => {
  const kept = [
    { id: 'p1', name: 'Review crew', members: [{ name: 'codex', agent: 'codex', count: 2 }] },
    { id: '', name: 'no id' },
    { id: 'p2', name: '', members: [] },
    'junk',
  ];
  const list = presetsOf(kept);
  expect(list).toEqual([{ id: 'p1', name: 'Review crew', members: [{ name: 'codex', agent: 'codex', count: 2 }] }]);
  expect(presetsOf(undefined)).toEqual([]);
  // a new name adds one, with the members as they're sent (named, no panel-only words)
  const added = savePreset(list, '  GPU box ', [{ name: '', agent: 'opencode', machine: 'peer-1', modelLabel: 'Qwen' }], 'p9');
  expect(added.id).toBe('p9');
  expect(added.updated).toBe(false);
  expect(added.list[1]).toEqual({ id: 'p9', name: 'GPU box', members: [{ name: 'opencode', agent: 'opencode', machine: 'peer-1' }] });
  // the same name, any case: that preset is updated in place, its id kept
  const same = savePreset(added.list, 'review CREW', [{ name: 'r', agent: 'claude' }], 'p10');
  expect(same.id).toBe('p1');
  expect(same.updated).toBe(true);
  expect(same.list.map((p) => p.id)).toEqual(['p1', 'p9']);
  expect(same.list[0]?.members).toEqual([{ name: 'r', agent: 'claude' }]);
  expect(same.list[0]?.name).toBe('review CREW');
  expect(deletePreset(same.list, 'p1').map((p) => p.id)).toEqual(['p9']);
  // the setting's value as the server keeps it: plain JSON
  expect(JSON.parse(JSON.stringify(same.list))).toEqual(same.list);
});
