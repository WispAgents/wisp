// The composer's `+` (docs/ux/2026-11-18-desktop.md #3): Attach files, Start from an issue…, Recipes ›, then Goal…,
// Delegate…, Compare with another agent…, then Start later…. Each row keeps its control's part (no spell breaks), a
// control with its own menu is a submenu, and a pick does what the control did.
import { describe, expect, test, vi } from 'vitest';
import { plusTool } from '../src/frame/composer-plus.ts';

const attach = vi.fn();
const issue = vi.fn();
const goal = vi.fn();
const delegate = vi.fn();
const compare = vi.fn();
const later = vi.fn();
const recipe = vi.fn();
const plus = plusTool({
  part: 'launcher.plus',
  label: 'Add',
  title: 'Add files, a goal, delegates…',
  sections: [
    [
      { id: 'attach', part: 'launcher.attach', icon: 'clip', label: 'Attach files', keys: '⌘U', act: { onClick: attach } },
      { id: 'issue', part: 'launcher.issue', icon: 'issue', label: 'Start from an issue…', act: { onClick: issue } },
      { id: 'recipes', part: 'launcher.recipes', icon: 'bolt', label: 'Recipes', act: { menu: { items: [{ value: 'recipe:r1', label: 'Release' }, { value: 'recipe:new', label: 'New recipe' }], onSelect: recipe } } },
    ],
    [
      { id: 'goal', part: 'launcher.goal', icon: 'target', label: 'Goal…', act: { onClick: goal } },
      { id: 'team', part: 'launcher.team', icon: 'agent', label: 'Delegate…', act: { onClick: delegate } },
      { id: 'compare', part: 'launcher.compare', icon: 'split', label: 'Compare with another agent…', act: { onClick: compare } },
      false,
    ],
    [{ id: 'later', part: 'launcher.later', icon: 'clock', label: 'Start later', act: { menu: { items: [{ value: '10m', label: 'In 10 min' }], onSelect: later } } }],
  ],
});

describe('the composer’s +_ux_2026_11_18_3', () => {
  test('one tool, its rows in sections, each with its control’s part', () => {
    expect([plus.id, plus.part, plus.icon]).toEqual(['plus', 'launcher.plus', 'plus']);
    const items = plus.act?.menu?.items ?? [];
    expect(items.map((e) => ('value' in e ? e.value : e.kind))).toEqual(['attach', 'issue', 'recipes', 'separator', 'goal', 'team', 'compare', 'separator', 'later']);
    expect(items.map((e) => ('part' in e ? e.part : undefined)).filter(Boolean)).toEqual(['launcher.attach', 'launcher.issue', 'launcher.recipes', 'launcher.goal', 'launcher.team', 'launcher.compare', 'launcher.later']);
    expect(items[0]).toMatchObject({ label: 'Attach files', keys: '⌘U' });
    expect(items[2]).toMatchObject({ kind: 'submenu', label: 'Recipes', items: [{ value: 'recipe:r1' }, { value: 'recipe:new' }] });
    expect(items[8]).toMatchObject({ kind: 'submenu', label: 'Start later' });
  });

  test('a pick does what the control did, a submenu’s choice goes to its own menu', () => {
    const pick = plus.act?.menu?.onSelect ?? (() => undefined);
    for (const v of ['attach', 'issue', 'goal', 'team', 'compare', 'recipe:r1', '10m']) pick(v);
    expect([attach, issue, goal, delegate, compare].map((f) => f.mock.calls.length)).toEqual([1, 1, 1, 1, 1]);
    expect(recipe).toHaveBeenCalledWith('recipe:r1');
    expect(later).toHaveBeenCalledWith('10m');
  });

  test('an empty section leaves no line', () => {
    const p = plusTool({ part: 'composer.plus', label: 'Add', title: 'Add', sections: [[], [{ id: 'goal', part: 'composer.goal', icon: 'target', label: 'Goal…', act: {} }], []] });
    expect((p.act?.menu?.items ?? []).map((e) => ('value' in e ? e.value : e.kind))).toEqual(['goal']);
  });
});
