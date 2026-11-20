// The phone apps' New task bar (QA r2 #2): Goal, Worktree and Start later sat past the screen's edge or under Send, and
// Recipes were gone. On a phone they fold into one More menu (ADR-0012: a bar collapses, it never runs off), with
// Start from an issue and Recipes, so every option is one tap away.
import { describe, expect, test, vi } from 'vitest';
import type { LauncherTool } from '@wisp/ui';
import { foldTail } from '../src/frame/phone-bar.ts';
import { goalAct } from '../src/frame/goal-menu.ts';

const goal = vi.fn();
const worktree = vi.fn();
const later = vi.fn();
const issue = vi.fn();
const recipe = vi.fn();
const tools: LauncherTool[] = [
  { id: 'goal', part: 'launcher.goal', icon: 'target', label: 'Goal', title: 'Goal', on: true, act: { onClick: goal } },
  { id: 'worktree', part: 'launcher.worktree', icon: 'branch', label: 'Worktree', title: 'Worktree', act: { onClick: worktree } },
  { id: 'later', part: 'launcher.later', icon: 'clock', label: 'Start later', title: 'Start later', act: { menu: { items: [{ value: '10m', label: 'In 10 min' }, { value: 'morning', label: 'Tomorrow morning' }], onSelect: later } } },
];
const more = foldTail(tools, {
  label: 'More',
  title: 'More options',
  issue: { label: 'Start from an issue', act: { onClick: issue } },
  recipes: { label: 'Recipes', items: [{ id: 'r1', label: 'Release', act: { onClick: () => recipe('r1') } }], add: { label: 'New recipe', act: { onClick: () => recipe('new') } } },
});

describe('the phone bar folds its tail into More_qa_r2_2', () => {
  test('one tool, a menu: the toggles with their state, Start later and Recipes as submenus, the issue picker', () => {
    expect(more.id).toBe('more');
    expect(more.icon).toBe('dots');
    const items = more.act?.menu?.items ?? [];
    expect(items.map((e) => ('value' in e ? e.value : e.kind))).toEqual(['tool:goal', 'tool:worktree', 'tool:later', 'separator', 'issue', 'recipes']);
    expect(items[0]).toMatchObject({ label: 'Goal', icon: 'target', checked: true });
    expect(items[2]).toMatchObject({ kind: 'submenu', label: 'Start later', items: [{ value: '10m' }, { value: 'morning' }] });
    expect(items[5]).toMatchObject({ kind: 'submenu', label: 'Recipes' });
  });

  test('a pick does what the tool did', () => {
    const pick = more.act?.menu?.onSelect ?? (() => {});
    pick('tool:goal');
    pick('tool:worktree');
    pick('morning');
    pick('issue');
    pick('recipe:r1');
    pick('recipe:new');
    expect(goal).toHaveBeenCalledOnce();
    expect(worktree).toHaveBeenCalledOnce();
    expect(later).toHaveBeenCalledWith('morning');
    expect(issue).toHaveBeenCalledOnce();
    expect(recipe.mock.calls).toEqual([['r1'], ['new']]);
  });

  test('it shows as on while any folded option is on, so a set goal or time is never hidden', () => {
    expect(more.on).toBe(true);
    expect(foldTail([{ ...tools[1]! }], { label: 'More', title: 'More' }).on).toBeFalsy();
  });
});

// QA phone r4 #1: the pill grew to "Claude Code ▾ +" and pushed More under Send (Goal, Worktree, Start later and Recipes
// out of reach). Try another agent (the pill's +) is in More too, so a tight bar can fold the + and lose nothing
describe('the phone bar folds the pill before More_qa_r4_1', () => {
  test('More holds Try another agent, first of the extras', () => {
    const compare = vi.fn();
    const m = foldTail(tools, { label: 'More', title: 'More', compare: { label: 'Try another agent', act: { onClick: compare } }, issue: { label: 'Start from an issue', act: { onClick: issue } } });
    const items = m.act?.menu?.items ?? [];
    expect(items.map((e) => ('value' in e ? e.value : e.kind))).toEqual(['tool:goal', 'tool:worktree', 'tool:later', 'separator', 'compare', 'issue']);
    m.act?.menu?.onSelect('compare');
    expect(compare).toHaveBeenCalledOnce();
  });
});

// Audit G tasks #2 (owner-list #61, #63): the phone apps' More had a plain Goal toggle and no Formula or task file…; it
// offers desktop's Goal… (the declared goals, a filter past seven, All goals…, New goal…). Folded, its filter and the
// goals past the first seven must survive
describe('the_phone_more_offers_goal_with_the_declared_goals_and_its_filter_audit_g_tasks_2', () => {
  test('Goal… folds as a submenu that keeps its filter, and a goal the filter finds is taken', () => {
    const declared = Array.from({ length: 32 }, (_, i) => ({ name: `goal-${String(i + 1).padStart(2, '0')}`, text: `done when ${i + 1}` }));
    const taken: string[] = [];
    const t = (k: string, v?: Readonly<Record<string, string | number>>): string => (k === 'shell.home.goalAll' ? `All goals (${v?.n})…` : k === 'shell.home.goalNew' ? 'New goal…' : k);
    const act = goalAct({ shell: { navigate: () => undefined } as never, declared, recent: () => [], t, take: (g) => taken.push(g.name), fresh: () => taken.push('fresh') });
    const formula = vi.fn();
    const folded = foldTail(
      [
        { id: 'goal', part: 'launcher.goal', icon: 'target', label: 'Goal…', title: 'Goal', act },
        { id: 'formula', part: 'launcher.formula', icon: 'file', label: 'Formula or task file…', title: 'Formula or task file…', act: { onClick: formula } },
      ],
      { label: 'More', title: 'More options' },
    );
    const items = folded.act?.menu?.items ?? [];
    const goal = items[0] as { kind: string; items: { value: string; label: string }[]; filter?: { all: unknown[] } };
    expect(goal.kind).toBe('submenu');
    expect(goal.items.map((i) => i.label).slice(-2)).toEqual(['All goals (32)…', 'New goal…']);
    expect(goal.filter?.all.length, 'the filter over all 32').toBe(32);
    folded.act?.menu?.onSelect('goal:goal-17');
    folded.act?.menu?.onSelect('goal:new');
    expect(taken).toEqual(['goal-17', 'fresh']);
    expect(items[1]).toMatchObject({ value: 'tool:formula', label: 'Formula or task file…' });
    folded.act?.menu?.onSelect('tool:formula');
    expect(formula).toHaveBeenCalled();
  });
});
