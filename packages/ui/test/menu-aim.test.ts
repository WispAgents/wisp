// The safe triangle (./src/controls/menu-aim.ts): from where the pointer left a submenu's row to its flyout's near edge.
import { describe, expect, test } from 'vitest';
import { aimTriangle, emptyBox, inBox, inTriangle } from '../src/controls/menu-aim.ts';

const fly = { left: 360, top: 400, right: 580, bottom: 520 };

describe('aimTriangle', () => {
  test('a flyout to the right: the triangle reaches its left edge_aim_right', () => {
    expect(aimTriangle({ x: 260, y: 450 }, fly)).toEqual([{ x: 260, y: 450 }, { x: 360, y: 400 }, { x: 360, y: 520 }]);
  });
  test('a flyout flipped to the left: the triangle reaches its right edge_aim_left', () => {
    expect(aimTriangle({ x: 700, y: 450 }, fly)).toEqual([{ x: 700, y: 450 }, { x: 580, y: 400 }, { x: 580, y: 520 }]);
  });
});

describe('inTriangle', () => {
  const t = aimTriangle({ x: 260, y: 455 }, fly);
  test('a diagonal from the row to the flyout’s lower items stays inside all the way_aim_diagonal', () => {
    const end = { x: 360, y: 510 };
    for (let i = 1; i <= 20; i++) {
      const p = { x: 260 + ((end.x - 260) * i) / 20, y: 455 + ((end.y - 455) * i) / 20 };
      expect(inTriangle(p, t), `${p.x},${p.y}`).toBe(true);
    }
  });
  test('straight down to the next rows is outside: it isn’t heading for the flyout_aim_down', () => {
    expect(inTriangle({ x: 262, y: 500 }, t)).toBe(false);
    expect(inTriangle({ x: 200, y: 455 }, t)).toBe(false);
  });
  test('either winding, edges included_aim_winding', () => {
    const [a, b, c] = t;
    expect(inTriangle(b, [a, c, b])).toBe(true);
    expect(inTriangle({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }, t)).toBe(true);
  });
});

test('inBox and emptyBox_aim_boxes', () => {
  expect(inBox({ x: 360, y: 400 }, fly)).toBe(true);
  expect(inBox({ x: 359, y: 400 }, fly)).toBe(false);
  expect(emptyBox({ left: 0, top: 0, right: 0, bottom: 0 })).toBe(true);
  expect(emptyBox(fly)).toBe(false);
});
