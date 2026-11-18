import { expect, test } from 'vitest';
import { accountChoices, accountFor, defaultWord, effortsFor, harnessesFor, keepOrder, modelsFor, offerState, offersFrom, runnerWords, type AccountOffer, type AgentOffer, type HarnessTopicRow, runPick, fullest, usedPct } from '../src/frame/runner.ts';

const AGENTS: AgentOffer[] = [
  { id: 'codex', name: 'Codex', available: true, models: [{ id: 'gpt-5.5', label: 'GPT-5.5' }], efforts: ['low', 'high'] },
  { id: 'cursor', name: 'Cursor', available: false, models: [], efforts: [] },
  { id: 'claude', name: 'Claude Code', available: true, models: [{ id: 'opus', label: 'Opus 5.5', efforts: ['low', 'medium', 'high', 'max'] }, { id: 'haiku', label: 'Haiku 5.5' }], efforts: ['low', 'high'] },
];

test('the_harness_model_picker_searches_every_harnesses_models_and_offers_the_picks_thinking_levels', () => {
  // set up ones first; a model's name finds its harness
  expect(harnessesFor(AGENTS, '').map((a) => a.id)).toEqual(['codex', 'claude', 'cursor']);
  expect(harnessesFor(AGENTS, 'opus').map((a) => a.id)).toEqual(['claude']);
  // the Default first; a query narrows, the harness's own name keeps them all
  expect(modelsFor(AGENTS, 'claude', '').map((m) => m.id)).toEqual(['', 'opus', 'haiku']);
  expect(modelsFor(AGENTS, 'claude', 'hai').map((m) => m.id)).toEqual(['haiku']);
  expect(modelsFor(AGENTS, 'claude', 'claude').length).toBe(3);
  expect(modelsFor(AGENTS, 'nope', '')).toEqual([]);
  // a model's own levels, else the harness's
  expect(effortsFor(AGENTS, { agent: 'claude', model: 'opus', effort: '' })).toEqual(['low', 'medium', 'high', 'max']);
  expect(effortsFor(AGENTS, { agent: 'claude', model: 'haiku', effort: '' })).toEqual(['low', 'high']);
  expect(runnerWords(AGENTS, { agent: 'claude', model: 'opus', effort: 'high' }, (id) => (id === 'claude' ? 'Claude Code' : id))).toBe('Claude Code · Opus 5.5 · high');
});

test('the_reply_boxs_harness_model_pick_goes_with_the_next_follow_up_and_none_keeps_the_tasks_own', () => {
  expect(runPick(undefined)).toBeUndefined();
  // a pick names its model and thinking, '' its harness's default (QA desktop r3 #1: dropped, "Default" kept the
  // task's Opus, and another harness was asked for the old one's model)
  expect(runPick({ agent: 'codex', model: '', effort: '' })).toEqual({ agent: 'codex', model: '', effort: '' });
  expect(runPick({ agent: 'claude', model: 'opus', effort: 'high' })).toEqual({ agent: 'claude', model: 'opus', effort: 'high' });
});

// F-comp-38: before you pick, each harness says how much of its plan is gone: its fullest window over its accounts
// (v1 runner-picker.js fullest/usageNote), whole percents rounded down; none known, no line.
test('the harness picker shows each harness fullest plan window_v1_picker_usage', () => {
  const w = fullest([
    { windows: [{ label: '5h', used: 0.42 }, { label: 'Week', used: 0.31 }] },
    { windows: [{ label: '5h', used: 0.879 }] },
  ]);
  expect(w?.label).toBe('5h');
  expect(w && usedPct(w)).toBe(87);
  expect(fullest([])).toBeUndefined();
  expect(fullest([{ windows: [] }])).toBeUndefined();
});

// F-comp-41: a harness that's installed but signed out sits with the set-up ones, so it can be picked to sign in
// right there (v1 runner-picker.js signedOut / paintSignIn).
test('a signed out harness sits with the set up ones so it can be picked to sign in_v1_picker_sign_in', () => {
  const list: AgentOffer[] = [
    { id: 'cursor', name: 'Cursor', available: false, models: [], efforts: [] },
    { id: 'codex', name: 'Codex', available: false, signedIn: false, models: [], efforts: [] },
    { id: 'claude', name: 'Claude Code', available: true, models: [], efforts: [] },
  ];
  expect(harnessesFor(list, '').map((a) => a.id)).toEqual(['codex', 'claude', 'cursor']);
});

const WORDS = { auto: 'Auto', autoHint: 'saved', limit: 'at limit', out: 'signed out' };

test('a_second_account_shows_in_the_picker_and_the_task_starts_on_the_one_picked', () => {
  const one: AccountOffer[] = [{ id: 'default', label: 'Main', default: true, state: 'ready' }];
  // one account: no choice (v1 accountsAt: two or more)
  expect(accountChoices(one, WORDS)).toEqual([]);
  expect(accountFor(one, 'default')).toBe('');
  const two: AccountOffer[] = [
    { id: 'default', label: 'Main', email: 'me@home.dev', plan: 'Max', default: true, state: 'ready' },
    { id: 'w1', label: 'Work', state: 'limit', until: 9 },
    { id: 'p2', label: 'Spare', state: 'signedOut' },
  ];
  const rows = accountChoices(two, WORDS);
  // Auto first, then your order; a signed-out one can't be picked, one at its limit can and says so
  expect(rows.map((r) => r.value)).toEqual(['', 'default', 'w1', 'p2']);
  expect(rows[1]).toEqual({ value: 'default', label: 'Main', hint: 'me@home.dev · Max' });
  expect(rows[2]).toEqual({ value: 'w1', label: 'Work', hint: 'at limit' });
  expect(rows[3]?.disabled).toBe(true);
  expect(accountFor(two, 'w1')).toBe('w1');
  expect(accountFor(two, 'p2')).toBe('');
  expect(accountFor(two, 'gone')).toBe('');
  // the bar says which account
  expect(runnerWords(AGENTS, { agent: 'codex', model: '', effort: '', account: 'w1', accountLabel: 'Work' }, (id) => id)).toBe('codex · Work');
});

// ADR-0066: the picker renders from the `harnesses` topic before any CLI answers, a row per harness that says
// Checking… until its own detect lands, and nothing moves while the picker is open.
test('the_picker_shows_every_harness_at_once_as_checking_and_each_row_settles_in_place', () => {
  const row = (id: string, ord: number, state: HarnessTopicRow['state'], available = false): HarnessTopicRow => ({ id, ord, state, agent: { id, name: id.toUpperCase(), available, models: [], efforts: [] } });
  const answered: AgentOffer[] = [{ id: 'old', name: 'Old', available: true, models: [], efforts: [] }];
  // a wisp with no such topic: what runtime.agents answered
  expect(offersFrom([], answered)).toBe(answered);
  expect(offersFrom([], undefined)).toEqual([]);
  // the topic wins, in its order, and marks the ones still checking
  const first = offersFrom([row('b', 1, 'checking'), row('a', 0, 'checking'), row('c', 2, 'checking')], answered);
  expect(first.map((a) => [a.id, offerState(a)])).toEqual([
    ['a', 'checking'],
    ['b', 'checking'],
    ['c', 'checking'],
  ]);
  // checking sits with the set-up ones, so it doesn't start below a missing one
  const opened = harnessesFor([...first, { id: 'gone', name: 'Gone', available: false, models: [], efforts: [] }], '');
  expect(opened.map((a) => a.id)).toEqual(['a', 'b', 'c', 'gone']);
  // b and c settle: b missing, c ready. A fresh sort would move c above b; kept, every row stays where it was
  const settled = offersFrom([row('a', 0, 'checking'), row('b', 1, 'missing'), row('c', 2, 'installed', true)], undefined);
  const shown = opened.map((a) => a.id);
  expect(harnessesFor([...settled], '').map((a) => a.id)).toEqual(['a', 'c', 'b']);
  expect(keepOrder(shown, harnessesFor([...settled], '')).map((a) => a.id)).toEqual(['a', 'b', 'c']);
  expect(settled.map((a) => offerState(a))).toEqual(['checking', 'missing', 'ready']);
  // a new harness goes after the shown ones
  expect(keepOrder(['b', 'a'], [...offersFrom([row('a', 0, 'installed', true), row('z', 1, 'checking'), row('b', 2, 'missing')], undefined)]).map((a) => a.id)).toEqual(['b', 'a', 'z']);
  // signed out stays its own state, checking first
  expect(offerState({ available: false, signedIn: false })).toBe('signedOut');
  expect(offerState({ available: true, signedIn: false, checking: true })).toBe('checking');
});

// UX 2026-11-18 #9: the chip names the model Default runs when the harness can tell (its own config, read by its plugin:
// HarnessModel.resolves), else it says Default
test('the_default_is_named_by_the_model_it_runs_when_the_harness_says_ux_2026_11_18_9', () => {
  const row = (id: string, resolves?: string): HarnessTopicRow => ({
    id,
    ord: 0,
    state: 'installed',
    agent: { id, name: id, available: true, models: [{ id: '', label: 'Default', ...(resolves && { resolves }) }, { id: 'opus', label: 'Opus' }], efforts: [] },
  });
  const rows = [row('claude', 'Opus'), row('codex')];
  expect(defaultWord(rows, 'claude', 'Default')).toBe('Opus');
  expect(defaultWord(rows, 'codex', 'Default')).toBe('Default');
  expect(defaultWord(rows, 'cursor', 'Default')).toBe('Default');
  expect(defaultWord([], 'claude', 'Default')).toBe('Default');
});
