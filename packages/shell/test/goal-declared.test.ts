import { expect, test } from 'vitest';
import { goalWire } from '../src/frame/goal-gate.ts';
import { declaredGoals, goalMenu, pickedGoal } from '../src/frame/goal-menu.ts';

const say = (k: string, v?: Readonly<Record<string, string | number>>): string => (k === 'shell.home.goalNew' ? 'New goal…' : `${k}${v ? JSON.stringify(v) : ''}`);

const green = { name: 'green-build', text: 'npm test passes' };

// ADR-0110 (owner 2026-11-19): New task's Goal… offers the goals declared by name, or a new one to write
test('goal_row_offers_the_declared_goals_by_name_then_a_new_one', () => {
  expect(goalMenu([green], [], say)).toEqual({
    items: [
      { value: 'goal:green-build', label: 'green-build', hint: 'npm test passes', icon: 'target' },
      { value: 'goal:new', label: 'New goal…', icon: 'plus' },
    ],
  });
  expect(goalMenu([], [], say)).toEqual({ items: [{ value: 'goal:new', label: 'New goal…', icon: 'plus' }] });
  expect(pickedGoal([green], 'goal:green-build')).toEqual(green);
  expect(pickedGoal([green], 'goal:new')).toBeUndefined();
});

test('a_picked_declared_goal_is_sent_by_name_until_done_when_is_edited', () => {
  expect(goalWire('npm test passes', green)).toEqual({ name: 'green-build' });
  expect(goalWire('npm test passes and lint', green)).toBe('npm test passes and lint');
  expect(goalWire('the tests pass', undefined)).toBe('the tests pass');
  expect(goalWire(undefined, green)).toBeUndefined();
});

test('declared_goals_read_from_the_peer_keep_only_named_goals_with_text', () => {
  expect(declaredGoals([{ name: 'green-build', text: 'npm test passes', judges: ['claude'] }, { name: '', text: 'x' }, { text: 'y' }, 3])).toEqual([green]);
  expect(declaredGoals(undefined)).toEqual([]);
});
