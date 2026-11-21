// Ask near the limit's sentence (hold.ts; v1 features/hold.js `whose` + `landing`, limit-hold.js `holdTo`).
import { expect, test } from 'vitest';
import { holdSay, holdTo } from '../src/hold.ts';

const h = (used: number, to: [number, number], label = 'Weekly', agent = 'claude', line = 0.9) => ({ agent, label, used, line, to });

test('past the line already: says so with the line (v1: "…, past your 90% line.")', () => {
  expect(holdSay(h(0.91, [0.91, 0.912]), false)).toBe('Claude Code’s weekly window is at 91%, past your 90% line.');
});

test('a new task says where it may take the window; a follow-up says where one like it usually does', () => {
  expect(holdSay(h(0.85, [0.93, 0.97], '5-hour'), false)).toBe('Claude Code’s 5-hour window is at 85%. This task may take it to 93–97%.');
  expect(holdSay(h(0.85, [0.95, 0.96], '5-hour', 'codex'), true)).toBe('Codex’s 5-hour window is at 85%. A follow-up like this usually takes it to about 96%.');
});

test('past the limit, and the account named only when the harness has several', () => {
  expect(holdSay(h(0.95, [1.02, 1.1]), false, 'Work')).toBe('Claude Code (Work)’s weekly window is at 95%. This task may take it past the limit.');
});

test('holdTo matches v1: a range when wide, about N% when narrow (never 100), past the limit', () => {
  expect(holdTo([0.94, 0.99])).toBe('94–99%');
  expect(holdTo([0.995, 0.999])).toBe('about 99%');
  expect(holdTo([1, 1.2])).toBe('past the limit');
});
