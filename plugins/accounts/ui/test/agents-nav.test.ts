// Settings → Agents as a list and a detail (agents-nav.ts, harness audit r2): General first, then each harness with
// its state dot; the search narrows by a harness's name and by the settings its detail holds; nothing left: an empty
// list; the pick stays while the search hides it.
import { expect, test } from 'vitest';
import { GENERAL, modelProblem, navItems, shownItem, type NavHarness, type NavRows } from '../src/agents-nav.ts';

const ROWS: NavRows = {
  general: { label: 'General', rows: ['GitHub', 'git', 'Lean Claude Code runs', 'Re-check'] },
  harness: ['Sign in and accounts', 'Program', 'Use this harness', 'Default model', 'Thinking level', 'Mode'],
  atLimit: 'At its limit',
};
const HS: NavHarness[] = [
  { id: 'claude', name: 'Claude Code', state: { tone: 'good', label: 'Ready' } },
  { id: 'cursor', name: 'Cursor', state: { tone: 'warn', label: 'Signed out' } },
  { id: 'grok', name: 'Grok', state: { tone: 'bad', label: 'Not found' } },
  { id: 'codex', name: 'Codex', state: { tone: 'good', label: 'Ready' }, atLimit: true },
];

test('the_list_is_general_then_each_harness_with_its_state_dot', () => {
  const items = navItems(HS, '', ROWS);
  expect(items.map((i) => [i.id, i.dot ?? '', i.sub ?? ''])).toEqual([
    [GENERAL, '', ''],
    ['claude', 'good', 'Ready'],
    ['cursor', 'warn', 'Signed out'],
    ['grok', 'bad', 'Not found'],
    ['codex', 'warn', 'At its limit'],
  ]);
});

test('the_search_narrows_by_name_and_by_setting_title_and_can_leave_nothing', () => {
  expect(navItems(HS, 'cur', ROWS).map((i) => i.id)).toEqual(['cursor']);
  // a setting every harness's detail holds: every harness, not General
  expect(navItems(HS, 'default model', ROWS).map((i) => i.id)).toEqual(['claude', 'cursor', 'grok', 'codex']);
  // a harness and one of its settings
  expect(navItems(HS, 'grok mode', ROWS).map((i) => i.id)).toEqual(['grok']);
  // one of General's rows
  expect(navItems(HS, 'github', ROWS).map((i) => i.id)).toEqual([GENERAL]);
  expect(navItems(HS, 'xyzzy', ROWS)).toEqual([]);
});

test('the_pick_survives_a_search_that_hides_it_and_a_kept_model_it_no_longer_offers_is_an_error', () => {
  const narrowed = navItems(HS, 'cur', ROWS);
  expect(shownItem(narrowed, 'claude'), 'hidden: the first match shows').toBe('cursor');
  expect(shownItem(navItems(HS, '', ROWS), 'claude'), 'search cleared: the pick is back').toBe('claude');
  expect(shownItem([], 'claude')).toBeUndefined();
  const models = [{ id: 'opus' }, { id: 'sonnet' }];
  expect(modelProblem('opus', models, false)).toBeUndefined();
  expect(modelProblem('', models, false), 'Default is always offered').toBeUndefined();
  expect(modelProblem('gpt-1', models, false)).toBe('unknown');
  expect(modelProblem('gpt-1', models, true), 'still checking: not yet an error').toBeUndefined();
});
