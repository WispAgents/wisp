// The first-run launcher (src/frame/first-run.ts, v1 launcher.js firstRun): prompt, mode, harness, Send; the rest behind More.
import { expect, test } from 'vitest';
import { barMode, firstRunTools, type BarMode } from '../src/frame/first-run.ts';

const tool = (id: string) => ({ id, part: `launcher.${id}`, icon: 'clip' as const, label: id, title: id });

test('before your first task only the prompt, how it runs and Send show; More options opens the rest for good_v1_first_run', () => {
  const all = { full: false, mine: false, ready: true, loaded: true };
  expect(barMode('full', all)).toBe('first');
  expect(barMode('first', { ...all, full: true })).toBe('full'); // opened once: stays full
  expect(barMode('first', { ...all, mine: true })).toBe('full'); // a task of yours: full
  expect(barMode('full', { ...all, ready: false })).toBe('full'); // the saved answer isn't in yet: no flash for a returning user
  const more = tool('more');
  expect(firstRunTools(true, 'lead', [tool('attach'), tool('where')]).map((x) => x.id)).toEqual(['attach']);
  expect(firstRunTools(true, 'mid', [tool('team')])).toEqual([]);
  expect(firstRunTools(true, 'tail', [tool('goal'), tool('worktree'), tool('later')], more).map((x) => x.id)).toEqual(['more']);
  expect(firstRunTools(false, 'tail', [tool('goal'), tool('later')], more).map((x) => x.id)).toEqual(['goal', 'later']);
});

test('the bar keeps the mode it drew until the data says what it is, so it never flips while the stores load_adr_0017', () => {
  const d = { full: false, mine: false, ready: false, loaded: false };
  // the saved answer in before the task list: still what it drew (it went first run here, then full: a lost tap)
  expect(barMode('full', { ...d, ready: true })).toBe('full');
  // the list in before the saved answer: the same
  expect(barMode('full', { ...d, loaded: true })).toBe('full');
  // both in, none of yours: the first run, once
  expect(barMode('full', { ...d, ready: true, loaded: true })).toBe('first');
  // the list going again (a reset, a reconnect): the first run stays, no flash of the full bar
  expect(barMode('first', { ...d, ready: true })).toBe('first');
  // in every order the three can come in, the first run is drawn only from a whole list with none of yours, and once
  // your tasks are in it is never drawn again
  const steps: (keyof typeof d)[] = ['ready', 'loaded', 'mine'];
  const orders = (xs: (keyof typeof d)[]): (keyof typeof d)[][] => (xs.length <= 1 ? [xs] : xs.flatMap((x, i) => orders([...xs.slice(0, i), ...xs.slice(i + 1)]).map((r) => [x, ...r])));
  for (const order of orders(steps)) {
    let drawn: BarMode = 'full';
    let now = { ...d };
    for (const k of order) {
      now = { ...now, [k]: true };
      drawn = barMode(drawn, now);
      if (drawn === 'first') expect(now.ready && now.loaded && !now.mine, order.join(' → ')).toBe(true);
    }
    expect(drawn, order.join(' → ')).toBe('full');
  }
});
