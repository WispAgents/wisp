// Owner's desktop test 2026-11-17 #18: the task list and the side panel couldn't be resized. Their widths, as v1
// clamped them (split.js clampW, panel.js paintWidth).
import { expect, test } from 'vitest';
import { listWidth, panelWidth } from '../src/frame/pane-widths.ts';

test('the list is 220 to 720 wide, and leaves the task 420 beside it (never under 260)', () => {
  expect(listWidth(100, 1600, 64)).toBe(220);
  expect(listWidth(500, 1600, 64)).toBe(500);
  expect(listWidth(900, 1600, 64)).toBe(720);
  expect(listWidth(700, 1000, 64)).toBe(516);
  expect(listWidth(700, 600, 0)).toBe(260);
});

test('the panel is at least 360, and the task beside it keeps 480', () => {
  expect(panelWidth(200, 1600, 404)).toBe(360);
  expect(panelWidth(600, 1600, 404)).toBe(600);
  expect(panelWidth(1200, 1600, 404)).toBe(716);
  // the list hidden: the panel may take what it left
  expect(panelWidth(1200, 1600, 0)).toBe(1120);
  expect(panelWidth(900, 900, 404)).toBe(360);
  // a very wide window: never past what the setting keeps (shell.panel.width's max), or the save is refused
  expect(panelWidth(2400, 3000, 0)).toBe(1600);
});
