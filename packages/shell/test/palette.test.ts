import { expect, test } from 'vitest';
import type { PaletteItem } from '../src/contract.ts';
import { createRanker, fuzzy, marks, matchOne, rank, recency, splitQuery, tagScore } from '../src/palette/match.ts';

const item = (id: string, label: string, more: Partial<PaletteItem> = {}): PaletteItem => ({ id, group: 'g', label, ...more });

test('a word-start substring beats a scattered match', () => {
  const clean = matchOne('auth', 'Fix auth flow');
  const scattered = matchOne('auth', 'a unit test hub');
  expect(clean && scattered && clean.score > scattered.score).toBe(true);
  expect(matchOne('zz', 'abc')).toBeNull();
  expect(fuzzy('fix flow', ['Fix auth flow'])?.idx).toEqual([0, 1, 2, 9, 10, 11, 12]);
});

test('#tags filter; a plain word that is a tag counts', () => {
  expect(splitQuery('#auth login')).toEqual({ tags: ['auth'], text: 'login' });
  expect(tagScore(splitQuery('#auth'), ['db'])).toBeNull();
  expect(tagScore(splitQuery('#au'), ['auth'])?.score).toBe(8);
  const r = rank([item('1', 'Login page', { tags: ['auth'] }), item('2', 'Login page')], '#auth login', 0);
  expect(r.map((x) => x.item.id)).toEqual(['1']);
});

test('recency breaks ties and never reaches zero', () => {
  const now = 10 * 86_400_000;
  expect(recency(now, now)).toBe(20);
  expect(recency(now - 86_400_000, now)).toBeCloseTo(10);
  expect(recency(1, now)).toBeGreaterThan(0);
  const r = rank([item('old', 'Deploy', { at: 1 }), item('new', 'Deploy', { at: now })], 'deploy', now);
  expect(r[0]?.item.id).toBe('new');
});

test('empty query: each group shows its first 6 in the given order', () => {
  const items = [...Array.from({ length: 9 }, (_, i) => item(`a${i}`, `A${i}`, { group: 'A' })), item('b', 'B', { group: 'B' })];
  const r = rank(items, '', 0);
  expect(r.map((x) => x.item.id)).toEqual(['a0', 'a1', 'a2', 'a3', 'a4', 'a5', 'b']);
});

test("deep answers follow the rest and don't repeat listed items", () => {
  const r = rank([item('1', 'Refactor parser')], 'parser', 0, [item('1', 'Refactor parser'), item('2', 'Old parser work')]);
  expect(r.map((x) => x.item.id)).toEqual(['1', '2']);
  expect(marks('abc', [1])).toEqual([['a', false], ['b', true], ['c', false]]);
});

// audit C7: ⌘K "model" put "Theme: match the system" and "Dark theme" first: their keywords scatter it ("mode … colors")
// and that outscored Agents, whose lede has the word ("next to the model in the launcher"). A word match beats a
// scattered one, in any field; and the narrowing ranker answers as a fresh rank.
test('a whole word in any field beats letters scattered through another item', () => {
  const themeWords = 'theme appearance light dark mode system colors night';
  const items: PaletteItem[] = [
    item('shell.theme.system', 'Theme: match the system', { group: 'actions', keywords: themeWords }),
    item('shell.theme.dark', 'Dark theme', { group: 'actions', keywords: themeWords }),
    item('settings:agents', 'Agents', {
      group: 'settings',
      keywords: 'settings agents The harnesses wisp found on this machine. Pick one next to the model in the launcher.',
    }),
  ];
  expect(rank(items, 'model', 0)[0]?.item.id).toBe('settings:agents');
  const w = matchOne('model', 'next to the model in the launcher');
  const s = matchOne('model', themeWords);
  expect(w && s && w.score > s.score).toBe(true);
  const r = createRanker();
  for (const q of ['m', 'mo', 'mod', 'mode', 'model']) expect(r(items, q, 0).map((x) => x.item.id)).toEqual(rank(items, q, 0).map((x) => x.item.id));
});
