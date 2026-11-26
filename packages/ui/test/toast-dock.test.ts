// The toast stack keeps clear of the box at the bottom (toast-dock-scope.ts; v1 toast.js clearDock): B18.
import { expect, test } from 'vitest';
import { centreX, dockBottom, promptX, roomFor } from '../src/controls/toast-dock-scope.ts';

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

test('the stack centres on the main pane, not the window, so the sidebar never pulls it aside', () => {
  // sidebar open (320px): main spans 320..1440
  expect(centreX(1440, { left: 320, right: 1440, width: 1120 })).toBe(880);
  expect(roomFor(1440, { left: 320, right: 1440, width: 1120 })).toBe('1120px');
  // sidebar collapsed: main spans the whole window
  expect(centreX(1440, { left: 0, right: 1440, width: 1440 })).toBe(720);
  // no main pane drawn, or slid off-screen (phone stack on the list page): the window's middle
  expect(centreX(390, { left: 0, right: 0, width: 0 })).toBe(195);
  expect(centreX(390, { left: 390, right: 780, width: 390 })).toBe(195);
  expect(roomFor(390, { left: 390, right: 780, width: 390 })).toBe('100vw');
  expect(centreX(390, null)).toBe(195);
});

test('the dock is looked for under the stack centre, not the window middle', () => {
  // a reply box in the main pane (400..1100) is under a centre of 880, but not under one of 300
  expect(dockBottom(900, 1440, [box(804, 884)], 880)).toBe('106px');
  expect(dockBottom(900, 1440, [box(804, 884)], 300)).toBe('');
});

test('the stack centres on the prompt box, not the main pane, when one is drawn', () => {
  const main = { left: 320, right: 1440, width: 1120 };
  // a reply box at 400..1100 (a side pane on the right): centre 750, not the pane's 880
  expect(promptX(1440, main, [box(804, 884)])).toBe(750);
  // the lowest prompt box wins (a composer above a reply box)
  expect(promptX(1440, main, [box(300, 400, 500, 900), box(804, 884)])).toBe(750);
  // no prompt box, one not drawn, or one outside the main pane: the pane's middle
  expect(promptX(1440, main, [])).toBe(880);
  expect(promptX(1440, main, [{ top: 0, bottom: 0, left: 0, right: 0, width: 0 }])).toBe(880);
  expect(promptX(1440, main, [box(804, 884, 0, 300)])).toBe(880);
  // no main pane: a prompt box in the window still wins
  expect(promptX(390, null, [box(700, 800, 10, 370)])).toBe(190);
});
