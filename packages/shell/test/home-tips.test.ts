// UX 2026-11-18 desktop #8: Home showed a setup card (Grid) and three tips at once. It shows one, from one list: the
// setup cards first, then the tips; More tips moves one on and comes round again.
import { expect, test } from 'vitest';
import { tipPick } from '../src/frame/home-tips.ts';

test('home_shows_one_tip_at_a_time_setup_cards_among_them', () => {
  const tips = ['goal1', 'goal2', 'vs'];
  expect(tipPick(0, 1, tips)).toEqual({ kind: 'setup', i: 0 });
  expect(tipPick(1, 1, tips)).toEqual({ kind: 'tip', id: 'goal1' });
  expect(tipPick(3, 1, tips)).toEqual({ kind: 'tip', id: 'vs' });
  expect(tipPick(4, 1, tips), 'round again').toEqual({ kind: 'setup', i: 0 });
  expect(tipPick(0, 0, tips)).toEqual({ kind: 'tip', id: 'goal1' });
  expect(tipPick(5, 2, [])).toEqual({ kind: 'setup', i: 1 });
});
