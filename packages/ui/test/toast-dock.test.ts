// The toast stack keeps clear of the box at the bottom (toast-dock-scope.ts; v1 toast.js clearDock): B18.
import { expect, test } from 'vitest';
import { dockBottom } from '../src/controls/toast-dock-scope.ts';

const box = (top: number, bottom: number, left = 400, right = 1100) => ({ top, bottom, left, right, width: right - left });

test('a reply box at the bottom crossing the middle lifts the stack 10px over it', () => {
  expect(dockBottom(900, 1440, [box(804, 884)])).toBe('106px');
});

test('a box away from the bottom, off the middle, or not drawn leaves the stack where it is', () => {
  expect(dockBottom(900, 1440, [box(200, 400)])).toBe('');
  expect(dockBottom(900, 1440, [box(804, 884, 0, 300)])).toBe('');
  expect(dockBottom(900, 1440, [{ top: 0, bottom: 0, left: 0, right: 0, width: 0 }])).toBe('');
});

test('of several boxes, the highest one at the bottom wins', () => {
  expect(dockBottom(900, 1440, [box(804, 884), box(780, 884)])).toBe('130px');
});
