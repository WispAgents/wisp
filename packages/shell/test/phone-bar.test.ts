// The phone apps' New task bar (QA r2 #2): Goal, Worktree and Start later sat past the screen's edge or under Send, and
// Recipes were gone. On a phone they fold into one More menu (ADR-0012: a bar collapses, it never runs off), with
// Start from an issue and Recipes, so every option is one tap away.
import { describe, expect, test, vi } from 'vitest';
import type { LauncherTool } from '@wisp/ui';
import { foldTail } from '../src/frame/phone-bar.ts';

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
