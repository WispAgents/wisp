// The harness picker's model (src/frame/picker.ts, v1 runner-picker.js): the left pane's rows and sub-lines, the pane
// that shows on open, the right pane's model groups, and what each pick changes.
import { expect, test } from 'vitest';
import { firstFocus, goalOf, modelGroups, pickerRows, searchShown, stepRow, subLine, useHarness, varyRows, withModel, withSetting, withVary } from '../src/frame/picker.ts';
import { modelWord, type AgentOffer } from '../src/frame/runner.ts';
import { compareWords, newRun, runWords, sameSetup } from '../src/frame/trial.ts';
import { autoPick, runnerParts, runnerPick, type AutoPreview } from '../src/frame/auto.ts';

const CLAUDE: AgentOffer = {
  id: 'claude',
  name: 'Claude Code',
  available: true,
  models: [
    { id: 'opus', label: 'Opus 5.5', efforts: ['low', 'medium', 'high', 'max'] },
    { id: 'sonnet', label: 'Sonnet 5.5' },
    { id: 'haiku', label: 'Haiku 5.5' },
    { id: 'fable', label: 'Fable 5.2' },
  ],
  efforts: ['low', 'high'],
};
const CODEX: AgentOffer = { id: 'codex', name: 'Codex', available: true, models: [], efforts: [] };
const CURSOR: AgentOffer = { id: 'cursor', name: 'Cursor', available: false, models: [], efforts: [] };
const GROK: AgentOffer = { id: 'grok', name: 'Grok', available: false, signedIn: false, models: [{ id: 'g4', label: 'Grok 4' }], efforts: [] };
const OPENCODE: AgentOffer = {
  id: 'opencode',
  name: 'opencode',
  available: true,
  checking: true,
  models: [
    { id: 'anthropic/opus', label: 'Opus', hint: 'Anthropic' },
    { id: 'openai/gpt-5', label: 'GPT-5', hint: 'OpenAI' },
    { id: 'anthropic/sonnet', label: 'Sonnet', hint: 'Anthropic' },
  ],
  efforts: [],
};
const ALL = [CLAUDE, CODEX, CURSOR, GROK, OPENCODE];

test('the_left_pane_lists_auto_first_then_each_harness_with_its_sub_line_v1_runner_picker', () => {
  const rows = pickerRows(ALL, { auto: true, autoName: 'Auto', used: (id) => (id === 'codex' ? { label: '5h', pct: 42 } : undefined) });
  expect(rows.map((r) => [r.id, r.sub, r.off])).toEqual([
    ['auto', { k: 'auto' }, false],
    // "4 models": the Default isn't counted (v1 models.length - 1)
    ['claude', { k: 'models', n: 4 }, false],
    // how full its plan is, in place of the count (v1 usageNote)
    ['codex', { k: 'used', label: '5h', pct: 42 }, false],
    ['cursor', { k: 'missing' }, true],
    ['grok', { k: 'signedOut' }, true],
    // still detecting (ADR-0066): Checking…, and what was known still picks
    ['opencode', { k: 'checking' }, false],
  ]);
  expect(pickerRows(ALL, { auto: false, autoName: 'Auto' })[0]?.id).toBe('claude');
  expect(subLine({ ...CODEX, models: [] })).toEqual({ k: 'default' });
  // a never-seen harness still checking can't run yet: dimmed
  expect(pickerRows([{ ...CURSOR, checking: true }], { auto: false, autoName: 'Auto' })[0]?.off).toBe(true);
});

test('the_right_pane_opens_on_the_harness_in_use_else_the_first_usable_one', () => {
  const rows = pickerRows(ALL, { auto: true, autoName: 'Auto' });
  expect(firstFocus(rows, 'claude')).toBe('claude');
  expect(firstFocus(rows, 'auto')).toBe('auto');
  // one that can't run: the first usable one shows instead
  expect(firstFocus(rows, 'cursor')).toBe('auto');
  expect(firstFocus(pickerRows([CURSOR], { auto: false, autoName: 'Auto' }), 'x')).toBe('cursor');
  expect(firstFocus([], 'x')).toBe('');
  // the one in use while it's still checking: its pane, not one that switches under you once it lands
  expect(firstFocus(pickerRows([{ ...CURSOR, checking: true }, CODEX], { auto: true, autoName: 'Auto' }), 'cursor')).toBe('cursor');
});

test('arrows_go_round_the_rows', () => {
  const ids = ['auto', 'claude', 'codex'];
  expect([stepRow(ids, 'auto', 1), stepRow(ids, 'codex', 1), stepRow(ids, 'auto', -1), stepRow(ids, 'nope', 1), stepRow(ids, 'nope', -1), stepRow([], 'a', 1)]).toEqual(['claude', 'auto', 'codex', 'auto', 'codex', undefined]);
});

test('models_list_the_default_first_group_provider_ids_and_narrow_by_the_search', () => {
  expect(modelGroups(CLAUDE, '').map((g) => [g.head, g.items.map((m) => m.id)])).toEqual([['', ['', 'opus', 'sonnet', 'haiku', 'fable']]]);
  expect(modelGroups(CLAUDE, 'son').map((g) => g.items.map((m) => m.id))).toEqual([['sonnet']]);
  expect(modelGroups(OPENCODE, '').map((g) => [g.head, g.items.map((m) => m.id)])).toEqual([
    ['', ['']],
    ['Anthropic', ['anthropic/opus', 'anthropic/sonnet']],
    ['OpenAI', ['openai/gpt-5']],
  ]);
  // a group with nothing left goes
  expect(modelGroups(OPENCODE, 'gpt').map((g) => g.head)).toEqual(['OpenAI']);
  expect(modelGroups(undefined, '')).toEqual([{ head: '', items: [{ id: '', label: '' }] }]);
  // v1: the search shows past six
  expect([searchShown(CLAUDE), searchShown({ ...CLAUDE, models: [...CLAUDE.models, { id: 'a', label: 'A' }, { id: 'b', label: 'B' }] }), searchShown(undefined)]).toEqual([false, true, false]);
});

test('a_harness_press_uses_it_at_once_a_model_pick_carries_the_level_over_settings_commit_in_place', () => {
  const v = { agent: 'claude', model: 'opus', effort: 'max', account: 'work' };
  // the one in use, one not set up, one signed out: nothing changes (the pane says how to get it)
  expect([useHarness(v, 'claude', ALL), useHarness(v, 'cursor', ALL), useHarness(v, 'grok', ALL)]).toEqual([undefined, undefined, undefined]);
  expect(useHarness(v, 'codex', ALL)).toEqual({ agent: 'codex', model: '', effort: '', account: '' });
  expect(useHarness(v, 'auto', ALL)).toEqual({ agent: 'auto', model: '', effort: '', account: '' });
  // still checking but known ready: it picks
  expect(useHarness(v, 'opencode', ALL)?.agent).toBe('opencode');
  // max only on Opus: Sonnet takes the harness's levels, so the level goes back to Default
  // the pill's words: the model's name, never its id
  expect(withModel(v, 'claude', 'sonnet', ALL)).toEqual({ agent: 'claude', model: 'sonnet', effort: '', account: 'work', modelLabel: 'Sonnet 5.5' });
  expect(withModel({ ...v, effort: 'high' }, 'claude', 'sonnet', ALL).effort).toBe('high');
  // a model on another harness's pane moves to it, with its own account
  expect(withModel(v, 'codex', '', ALL)).toEqual({ agent: 'codex', model: '', effort: '', account: '' });
  expect(withModel(v, 'auto', 'cheap', ALL)).toEqual({ agent: 'auto', model: 'cheap', effort: '', account: '' });
  expect(withSetting(v, 'claude', { effort: 'low' })).toEqual({ ...v, effort: 'low' });
  expect(withSetting(v, 'codex', { account: 'b', accountLabel: 'B' })).toEqual({ agent: 'codex', model: '', effort: '', account: 'b', accountLabel: 'B' });
  expect([goalOf('cheap'), goalOf('nope'), goalOf(''), goalOf(undefined)]).toEqual(['cheap', 'balanced', 'balanced', 'balanced']);
});

test('vary_rows_subagents_show_the_setting_until_the_run_picks_its_own_skills_on_unless_off_only_what_the_harness_varies', () => {
  expect(varyRows([], undefined, '')).toEqual([]);
  expect(varyRows(['subagentModel', 'skills'], undefined, 'haiku')).toEqual([
    { id: 'subagentModel', options: ['', 'haiku', 'sonnet'], value: 'haiku' },
    { id: 'skills', options: ['on', 'off'], value: 'on' },
  ]);
  // the run's own pick wins over the setting, '' (the task's model) included
  expect(varyRows(['subagentModel'], { subagentModel: '' }, 'haiku')).toEqual([{ id: 'subagentModel', options: ['', 'haiku', 'sonnet'], value: '' }]);
  expect(varyRows(['skills'], { skills: 'off' }, '')).toEqual([{ id: 'skills', options: ['on', 'off'], value: 'off' }]);
});

test('with_vary_a_segment_changes_one_setting_and_keeps_the_other_skills_on_drops_its_key', () => {
  expect(withVary(undefined, 'subagentModel', 'haiku')).toEqual({ subagentModel: 'haiku' });
  expect(withVary({ subagentModel: 'haiku' }, 'skills', 'off')).toEqual({ subagentModel: 'haiku', skills: 'off' });
  expect(withVary({ subagentModel: 'haiku', skills: 'off' }, 'skills', 'on')).toEqual({ subagentModel: 'haiku' });
  expect(withVary({ skills: 'off' }, 'subagentModel', '')).toEqual({ skills: 'off', subagentModel: '' });
});

test('a_compare_run_is_a_copy_of_the_lead_only_on_the_same_harness_model_and_thinking_and_says_its_own_model', () => {
  const lead = { agent: 'claude', model: 'opus', effort: 'high' };
  expect(sameSetup({ ...lead }, lead)).toBe(true);
  expect(sameSetup({ ...lead, model: 'sonnet' }, lead)).toBe(false);
  expect(sameSetup({ ...lead, effort: '' }, lead)).toBe(false);
  expect(sameSetup({ ...lead, agent: 'codex' }, lead)).toBe(false);
  // its name's words: the model's name; thinking and account stay in the tooltip (v1 .runners.cmp)
  expect(compareWords({ agent: 'claude', model: 'opus', effort: 'high', modelLabel: 'Opus 5.5' }, 'Default')).toBe('Opus 5.5');
});

test('a_copy_of_the_lead_added_with_plus_starts_from_the_lead_s_model_thinking_and_account_so_try_it_two_ways_is_offered_at_once', () => {
  const lead = { agent: 'claude', model: 'opus', effort: 'high', account: 'w', modelLabel: 'Opus 5.5', accountLabel: 'Work' };
  const copy = newRun('claude', lead);
  expect(copy).toEqual(lead);
  expect(sameSetup(copy, lead)).toBe(true);
  // another harness: its own defaults, never the lead's model
  expect(newRun('codex', lead)).toEqual({ agent: 'codex', model: '', effort: '', account: '' });
});

test("Auto's pick segment says what v1 #autoPv says: the harness for its default model, else the model · level_auto_pick_segment_v1", () => {
  // v1 auto-preview.js paint(): <span class="apv-l">${shortLabel(r.pick)}</span>, shortLabel = model ? label minus the harness : label
  const preview = (pick: NonNullable<AutoPreview['pick']>): AutoPreview => ({ pick, why: '' });
  const say = (p: AutoPreview): string => runnerPick({ agent: 'auto', shown: autoPick(p)?.label, name: 'Claude Code', model: '', effort: '', account: '', fallback: 'Default' });
  // the router's default-model arm (server/router.js label `${a.name}`): the harness's name, whole (design/v1-reference/home.auto-preview)
  expect(say(preview({ agent: 'claude', model: '', effort: '', key: 'claude:', label: 'Claude Code' }))).toBe('Claude Code');
  // a model arm (label `${a.name} · ${m.label}` + level): the logo names the harness, the label the model · level; v1's .apv-l
  // caps it at 150px with an ellipsis, as app-home.css does
  expect(say(preview({ agent: 'codex', model: 'gpt-5.6-terra', effort: 'high', key: 'codex:gpt-5.6-terra:high', label: 'Codex · GPT-5.6-Terra · High' }))).toBe('GPT-5.6-Terra · High');
  // before Auto answers: the harness Auto runs
  expect(runnerPick({ agent: 'auto', name: 'Codex', model: '', effort: '', account: '', fallback: 'Default' })).toBe('Codex');
});

test('a_picked_harness_s_pick_comes_in_v1_s_parts_so_a_phone_keeps_the_harness_name_only_for_the_default_model_v1_rb_m_def', () => {
  // v1 runners.js: <span class="rb-m${m && m.id ? '' : ' def'}">${m ? m.label : 'Default'}</span>, then .rb-e, .rb-a;
  // launcher.css ≤520px hides .rb-h, .rb-m.def and .rb-e, and shows .rb-h again beside a .rb-m.def
  expect(runnerParts({ model: '', effort: '', account: '', fallback: 'Default' })).toEqual({ model: 'Default', def: true, effort: '', account: '' });
  expect(runnerParts({ model: 'Opus 4.7', effort: 'High', account: 'work', fallback: 'Default' })).toEqual({ model: 'Opus 4.7', def: false, effort: 'High', account: 'work' });
  // a level on the default model is still the default model: the phone shows the harness, the level goes to the picker
  expect(runnerParts({ model: '', effort: 'High', account: '', fallback: 'Default' }).def).toBe(true);
  // the wide pill's words are the parts joined, as before
  expect(runnerPick({ agent: 'claude', name: 'Claude Code', model: 'Opus 4.7', effort: 'High', account: 'work', fallback: 'Default' })).toBe('Opus 4.7 · High · work');
});

test('an_unknown_model_id_reads_default_on_the_harness_button_never_the_raw_id_v1_paint_runner_btn', () => {
  // v1 runners.js paintRunnerBtn: m = a.models.find(id); <span class="rb-m${m && m.id ? '' : ' def'}">${m && m.id ? m.label : 'Default'}</span>
  expect(modelWord({ model: '', modelLabel: '' })).toBe('');
  expect(modelWord({ model: 'opus', modelLabel: 'Opus 4.7' })).toBe('Opus 4.7');
  // an id no model of the harness has (a stale default, a model the CLI dropped): no label, so Default and def
  expect(modelWord({ model: 'gpt-4-legacy' })).toBe('');
  expect(modelWord({ model: 'gpt-4-legacy', modelLabel: '' })).toBe('');
  expect(runnerParts({ model: modelWord({ model: 'gpt-4-legacy' }), effort: 'High', account: '', fallback: 'Default' })).toEqual({ model: 'Default', def: true, effort: 'High', account: '' });
  expect(runnerPick({ agent: 'codex', name: 'Codex', model: modelWord({ model: 'gpt-4-legacy' }), effort: '', account: '', fallback: 'Default' })).toBe('Default');
});

test('a_compare_run_on_its_harness_s_default_model_reads_default_v1_paint_runner_btn_rb_m_def', () => {
  // v1 launcher.js paintCompare -> runners.js paintRunnerBtn: <span class="rb-m def">Default</span> on a .cmp-x button
  expect(compareWords({ agent: 'codex', model: '', effort: 'high', account: 'w' }, 'Default')).toBe('Default');
  // an id the harness doesn't list reads Default too, never the raw id
  expect(compareWords({ agent: 'codex', model: 'gpt-5', effort: '' }, 'Default')).toBe('Default');
  expect(compareWords({ agent: 'codex', model: 'gpt-5', effort: '', modelLabel: 'GPT-5' }, 'Default')).toBe('GPT-5');
});

test('a_copy_of_the_lead_says_its_model_until_a_change_is_picked_then_only_the_change_v1_same_needs_a_variant', () => {
  const copy = { agent: 'claude', model: '', effort: '' };
  // just added with +: no change picked yet, so it reads like any run (v1 "Claude Code Default"), never bare
  expect(runWords(copy, [], 'Default')).toEqual({ model: 'Default' });
  expect(runWords({ ...copy, model: 'opus', modelLabel: 'Opus 4.7' }, [], 'Default')).toEqual({ model: 'Opus 4.7' });
  // a change picked: the copy says only that (v1 .runners.trial .same)
  expect(runWords(copy, ['Subagents on Haiku'], 'Default')).toEqual({ variant: ['Subagents on Haiku'] });
});
