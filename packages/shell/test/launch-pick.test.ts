// New task keeps its harness, model and thinking level across navigation and reloads (owner #36, v1 launcher.js:
// S.config.defaults.agent / models / efforts, PATCH /config on each pick), and Auto is off behind one switch (owner #37).
import { expect, test } from 'vitest';
import { autoEnabled, autoOn, readLaunch, startRunner, withPick, withWhere } from '../src/frame/launch-pick.ts';

const ready = ['claude', 'codex', 'opencode'];

test('new_task_starts_on_the_harness_model_and_level_last_picked_owner_36', () => {
  const kept = withPick(withPick({}, 'codex', 'gpt-6', 'GPT-6', 'high'), 'claude', 'opus', 'Opus', 'medium');
  expect(kept.agent).toBe('claude');
  // each harness keeps its own model and level (v1 savedModel / savedEffort)
  expect(startRunner(kept, ready, false)).toEqual({ agent: 'claude', model: 'opus', modelLabel: 'Opus', effort: 'medium' });
  expect(startRunner({ ...kept, agent: 'codex' }, ready, false)).toEqual({ agent: 'codex', model: 'gpt-6', modelLabel: 'GPT-6', effort: 'high' });
  // the setting's value as the server keeps it (plain JSON) reads back the same
  expect(readLaunch(JSON.parse(JSON.stringify(kept)))).toEqual(kept);
});

test('with_nothing_picked_or_a_pick_gone_new_task_starts_on_the_first_ready_harness_owner_37', () => {
  expect(startRunner({}, ready, false)).toEqual({ agent: 'claude', model: '', modelLabel: '', effort: '' });
  expect(startRunner({ agent: 'grok' }, ready, false).agent).toBe('claude');
  // Auto off: an old Auto pick never comes back
  expect(startRunner({ agent: 'auto' }, ready, false).agent).toBe('claude');
  expect(readLaunch(null)).toEqual({});
  expect(readLaunch({ agent: 3, models: 'x' })).toEqual({});
});

test('auto_is_offered_only_when_its_one_switch_is_on_owner_37', () => {
  expect(autoEnabled(undefined)).toBe(false);
  expect(autoEnabled(false)).toBe(false);
  expect(autoEnabled('true')).toBe(false);
  expect(autoEnabled(true)).toBe(true);
  // on, Auto is what a fresh New task starts with, and a kept Auto pick stays
  expect(startRunner({}, ready, true).agent).toBe('auto');
  expect(startRunner({ agent: 'auto' }, ready, true).agent).toBe('auto');
  expect(startRunner({ agent: 'codex' }, ready, true).agent).toBe('codex');
});

test('a_plugin_reads_the_one_switch_from_its_wisps_setting_row_owner_37', () => {
  const rows = new Map<string, unknown>();
  const client = { home: 'home', entities: { row: (_e: string, k: string) => () => rows.get(k) } } as never;
  expect(autoOn(client)).toBe(false);
  rows.set('home:runtime.auto', { key: 'runtime.auto', value: false });
  expect(autoOn(client)).toBe(false);
  rows.set('home:runtime.auto', { key: 'runtime.auto', value: true });
  expect(autoOn(client)).toBe(true);
});

test('new_task_starts_on_the_computer_it_last_ran_on_v1_launch_where', () => {
  // Run on is kept beside the harness pick, and a later harness pick keeps it (and the other way round)
  const kept = withPick(withWhere(withPick({}, 'codex', 'gpt-6', 'GPT-6', 'high'), 'peer-mini'), 'claude', 'opus', 'Opus', 'medium');
  expect(kept.where).toBe('peer-mini');
  expect(kept.agent).toBe('claude');
  expect(withWhere(kept, 'peer-box').models).toEqual({ codex: 'gpt-6', claude: 'opus' });
  expect(readLaunch(JSON.parse(JSON.stringify(kept)))).toEqual(kept);
  // a malformed or empty place reads as none: New task runs on this computer
  expect(readLaunch({ where: 3 }).where).toBeUndefined();
  expect(readLaunch({ where: '' }).where).toBeUndefined();
});
