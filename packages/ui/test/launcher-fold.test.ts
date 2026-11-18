// The prompt box's toolbar on a narrow box (QA desktop r2 #4, design/layout.md §2): at 900 and 1024 px Start later sat
// half behind Send or past the bar's edge with no More holding it. The tail's last tools fold into More, as many as the
// measured widths need, never clipped, and at the edge it never flaps.
import { describe, expect, test } from 'vitest';
import { barPlan, tailFold } from '../src/app/launcher-fold.ts';

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

// QA desktop r3 #3 #4: at 1024 px WebKit's 520 px box took every label with 97 px to spare (v1's rule by width), and
// Chrome's kept them all and cut the model to "D…". The bar's labels go to their icons one step at a time, by what
// fits, before any tool goes into More, and the pick never truncates while a label can still go.
describe('the bar steps its labels to icons by measure, then folds_qa_desktop_r3_3_4', () => {
  const ids = ['goal', 'worktree', 'later'];
  const form = (others: number, tail: number[]) => ({ others, gap: 2, tail: new Map(ids.map((id, i) => [id, tail[i] ?? 30])) });
  // others with every label: 420 (the pick 222 in full); Goal's label off: 420; the mode's off too: 369; v1's form: 300
  const forms = [form(420, [61, 30, 30]), form(420, [30, 30, 30]), form(369, [30, 30, 30]), form(300, [30, 30, 30])];

  test('room for every label: none goes', () => {
    expect(barPlan({ available: 600, ids, forms, more: 30 })).toEqual({ level: 0, fold: 0 });
  });

  test('a little short: Goal goes to its icon first, then the mode, every tool still on the bar', () => {
    expect(barPlan({ available: 520, ids, forms, more: 30 })).toEqual({ level: 1, fold: 0 }); // 420 + 32 + 32 + 32 = 516
    expect(barPlan({ available: 500, ids, forms, more: 30 })).toEqual({ level: 2, fold: 0 }); // 369 + 96 = 465
  });

  test('narrower: v1’s narrow form, then the tail into More, the last first', () => {
    expect(barPlan({ available: 400, ids, forms, more: 30 })).toEqual({ level: 3, fold: 0 }); // 300 + 96 = 396
    expect(barPlan({ available: 380, ids, forms, more: 30 })).toEqual({ level: 3, fold: 2 }); // 300 + 32 + 32 = 364
    expect(barPlan({ available: 200, ids, forms, more: 30 })).toEqual({ level: 3, fold: 3 });
  });

  test('a level not measured yet is the next one tried, so the next measure knows it', () => {
    expect(barPlan({ available: 500, ids, forms: [forms[0]], more: 30 })).toEqual({ level: 1, fold: 0 });
    expect(barPlan({ available: 500, ids, forms: [], more: 30 })).toEqual({ level: 0, fold: 0 });
  });

  test('one width, one plan; and the plan never loosens as the bar narrows', () => {
    let last = { level: 0, fold: 0 };
    for (let w = 700; w > 150; w--) {
      const p = barPlan({ available: w, ids, forms, more: 30 });
      expect(p).toEqual(barPlan({ available: w, ids, forms, more: 30 }));
      expect(p.level * 10 + p.fold).toBeGreaterThanOrEqual(last.level * 10 + last.fold);
      last = p;
    }
  });
});
