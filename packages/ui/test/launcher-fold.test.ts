// The prompt box's toolbar on a narrow box (QA desktop r2 #4, design/layout.md §2): at 900 and 1024 px Start later sat
// half behind Send or past the bar's edge with no More holding it. The tail's last tools fold into More, as many as the
// measured widths need, never clipped, and at the edge it never flaps.
import { describe, expect, test } from 'vitest';
import { tailFold } from '../src/app/launcher-fold.ts';

// Goal 60, Worktree 90, Start later 40, gaps of 4, More 30: all three need 300 + 64 + 94 + 44 = 502
const bar = (available: number): number => tailFold({ available, others: 300, tail: [60, 90, 40], gap: 4, more: 30 });

describe('the tail folds into More by measured width_qa_desktop_r2_4', () => {
  test('room for every tool: nothing folds', () => {
    expect(bar(502)).toBe(0);
    expect(bar(1000)).toBe(0);
  });

  test('one pixel short: the last tool (Start later) goes into More, never clipped', () => {
    expect(bar(501)).toBe(1); // 300 + 64 + 94 + More 34 = 492
    expect(bar(492)).toBe(1);
  });

  test('narrower: Worktree follows, then Goal; with no room at all everything is in More', () => {
    expect(bar(491)).toBe(2); // 300 + 64 + 34 = 398
    expect(bar(398)).toBe(2);
    expect(bar(397)).toBe(3); // 300 + 34
    expect(bar(100)).toBe(3);
  });

  test('folding a tool no wider than More saves nothing, so it goes with the next', () => {
    expect(tailFold({ available: 491, others: 300, tail: [60, 90, 30], gap: 4, more: 30 })).toBe(2);
  });

  test('a width maps to one count, so a bar at the edge never flaps (the widths are the tools own, folded or not)', () => {
    for (let w = 250; w < 600; w++) expect(bar(w)).toBe(bar(w));
    // and the count never rises as the bar widens
    let last = 3;
    for (let w = 250; w < 600; w++) {
      const k = bar(w);
      expect(k).toBeLessThanOrEqual(last);
      last = k;
    }
  });

  test('no tail: nothing to fold', () => {
    expect(tailFold({ available: 10, others: 300, tail: [], gap: 4, more: 30 })).toBe(0);
  });
});
