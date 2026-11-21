// Owner-list #63 (owner 2026-11-19: "what if a user has a lot of goals, what will happen in that submenu?"): Goal…
// shows the goals this project used most recently first, then A–Z; about seven, then "All goals…" (a searchable picker),
// "New goal…" last; past seven a filter over all of them. Recipes take the same cap and filter in their own order.
import { afterEach, expect, test, vi } from 'vitest';
import { GOAL_ALL, GOAL_NEW, SHOWN, goalMenu, goalOrder, pickGoal, pickRecipe, pickedGoal, recentGoals, recipeMenu, RECIPE_ALL } from '../src/frame/goal-menu.ts';
import { pickDone, pickSpec } from '../src/frame/pick-list.ts';
import { plusTool } from '../src/frame/composer-plus.ts';
import { menuAt } from '../src/frame/prompt-menu.ts';
import type { MenuEntry, MenuSubmenu } from '@wisp/ui/pure';

const say = (k: string, v?: Readonly<Record<string, string | number>>): string => (v ? `${k}${JSON.stringify(v)}` : k);
const g = (name: string, text = `${name} holds`): { name: string; text: string } => ({ name, text });
// 30 declared goals, named out of order (the menu sorts them)
const thirty = Array.from({ length: 30 }, (_, i) => g(`goal-${String((i * 7) % 30).padStart(2, '0')}`));
const projects = [
  { id: 'p1', name: 'app', path: '/code/app', color: 'blue' },
  { id: 'p2', name: 'site', path: '/code/site', color: 'green' },
];
const task = (cwd: string, name: string | undefined, setAt: number) => ({ summary: { cwd, ...(name && { goal: { text: 'x', name, setAt } }) }, updated_at: setAt });

afterEach(() => vi.restoreAllMocks());

test('recent_goals_are_this_projects_most_recent_first_each_once', () => {
  const rows = [
    task('/code/app', 'goal-05', 10),
    task('/code/app/sub', 'goal-21', 30),
    task('/code/site', 'goal-09', 50), // another project's
    task('/code/app', 'goal-05', 40), // used again, later
    task('/code/app', undefined, 60), // a goal in words: no name
  ];
  expect(recentGoals(rows, projects, 'p1')).toEqual(['goal-05', 'goal-21']);
  expect(recentGoals(rows, projects, 'p2')).toEqual(['goal-09']);
  expect(recentGoals(rows, projects, undefined), 'no folder: no project to have used any').toEqual([]);
});

test('the_order_is_recent_first_then_a_to_z_any_case_and_a_recent_name_no_longer_declared_is_left_out', () => {
  const declared = [g('beta'), g('Alpha'), g('gamma'), g('delta')];
  expect(goalOrder(declared, ['gamma', 'gone', 'beta']).map((x) => x.name)).toEqual(['gamma', 'beta', 'Alpha', 'delta']);
  expect(goalOrder(declared, []).map((x) => x.name)).toEqual(['Alpha', 'beta', 'delta', 'gamma']);
});

test('seven_or_fewer_goals_show_every_one_then_new_goal_and_no_filter', () => {
  const seven = thirty.slice(0, SHOWN);
  const m = goalMenu(seven, [], say);
  expect(m.filter).toBeUndefined();
  expect(m.items).toHaveLength(SHOWN + 1);
  expect(m.items.at(-1)).toMatchObject({ value: GOAL_NEW, label: 'shell.home.goalNew' });
});

test('past_seven_the_first_seven_show_then_all_goals_then_new_goal_and_the_filter_searches_all_of_them', () => {
  const m = goalMenu(thirty, ['goal-17'], say);
  expect(SHOWN).toBe(7);
  expect(m.items.map((i) => i.value)).toEqual([
    'goal:goal-17', // used here most recently
    'goal:goal-00',
    'goal:goal-01',
    'goal:goal-02',
    'goal:goal-03',
    'goal:goal-04',
    'goal:goal-05',
    GOAL_ALL,
    GOAL_NEW,
  ]);
  expect(m.items[7]).toMatchObject({ label: 'shell.home.goalAll{"n":30}', icon: 'search' });
  expect(m.items[0]).toMatchObject({ label: 'goal-17', hint: 'goal-17 holds', icon: 'target' });
  expect(m.filter?.all).toHaveLength(30);
  expect(m.filter?.keep).toEqual([GOAL_ALL, GOAL_NEW]);
  expect(m.filter).toMatchObject({ label: 'shell.home.goalFilter', empty: 'shell.home.goalNone' });
  expect(pickedGoal(thirty, 'goal:goal-29')).toEqual(g('goal-29'));
  expect(pickedGoal(thirty, GOAL_ALL), 'All goals… is no goal').toBeUndefined();
});

test('all_goals_opens_the_searchable_picker_over_every_goal_in_the_menus_order_and_a_pick_takes_it', () => {
  const navigate = vi.fn();
  const take = vi.fn();
  pickGoal({ navigate }, thirty, ['goal-17'], say, take);
  const open = navigate.mock.calls[0]?.[0] as { t: string; overlay: { kind: string; id: string; params: { key: string } } };
  expect(open).toMatchObject({ t: 'openOverlay', overlay: { id: 'shell.pick', kind: 'dialog' } });
  const spec = pickSpec(open.overlay.params.key);
  expect(spec?.rows).toHaveLength(30);
  expect(spec?.rows[0]).toEqual({ value: 'goal:goal-17', label: 'goal-17', hint: 'goal-17 holds' });
  spec?.onPick('goal:goal-29');
  expect(take).toHaveBeenCalledWith(g('goal-29'));
  expect(pickDone(open.overlay.params.key)).toBe(true);
});

test('recipes_past_seven_keep_their_order_cap_at_seven_with_all_recipes_and_new_last', () => {
  const rows = Array.from({ length: 9 }, (_, i) => ({ value: `recipe:r${9 - i}`, label: `Recipe ${9 - i}` }));
  const m = recipeMenu(rows, 'New', say);
  expect(m.items.map((i) => i.value)).toEqual([...rows.slice(0, 7).map((r) => r.value), RECIPE_ALL, 'recipe:new']);
  expect(m.filter?.all).toHaveLength(9);
  expect(recipeMenu(rows.slice(0, 3), 'New', say).filter).toBeUndefined();
  const navigate = vi.fn();
  const run = vi.fn();
  pickRecipe({ navigate }, rows, say, run);
  const key = (navigate.mock.calls[0]?.[0] as { overlay: { params: { key: string } } }).overlay.params.key;
  pickSpec(key)?.onPick('recipe:r1');
  expect(run).toHaveBeenCalledWith('recipe:r1');
});

test('the_plus_menu_keeps_goals_filter_and_a_goal_the_filter_found_past_the_first_seven_is_picked', () => {
  const m = goalMenu(thirty, [], say);
  const onSelect = vi.fn();
  const warm = vi.fn();
  const tool = plusTool({
    part: 'launcher.plus',
    label: '+',
    title: 'More',
    onWarm: warm,
    sections: [[{ id: 'goal', part: 'launcher.goal', icon: 'target', label: 'Goal…', act: { menu: { items: m.items, ...(m.filter && { filter: m.filter }), onSelect } } }]],
  });
  const sub = tool.act?.menu?.items.find((e: MenuEntry): e is MenuSubmenu => e.kind === 'submenu');
  expect(sub?.filter?.all).toHaveLength(30);
  expect(tool.act?.menu?.onWarm, '`+` warms Goal…’s code').toBe(warm);
  // goal-29 isn't among the rows shown: the filter found it
  tool.act?.menu?.onSelect('goal:goal-29');
  expect(onSelect).toHaveBeenCalledWith('goal:goal-29');
});

test('slash_goal_and_one_word_reaches_the_sources_for_its_argument_but_a_goal_in_words_does_not', () => {
  expect(menuAt('/goal ', 6)).toEqual({ trigger: '/', start: 0, query: 'goal ' });
  expect(menuAt('/goal gre', 9)).toEqual({ trigger: '/', start: 0, query: 'goal gre' });
  expect(menuAt('/goal the tests pass', 20)).toBeNull();
  expect(menuAt('/goal\nmore', 10)).toBeNull();
});
