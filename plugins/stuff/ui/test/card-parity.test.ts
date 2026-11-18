// My Stuff's cards as v1 drew them (features/stuff.js paintCard, thumbBody, paintTop, emptySearch; the 2026-11-17
// parity audit, /tmp/qa-keep/stuff-parity.md #1–#8, #12): each fails on the v2 page as it was before.
import { expect, test } from 'vitest';
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { cardLook, headCount, launchLook, noneWhat, thumbOf } from '../src/card.ts';
import { fmtSize } from '../src/model.ts';
import type { Item } from '../src/types.ts';

const S = lazy as Record<string, string>;
const t = (k: string, v: Readonly<Record<string, string | number>> = {}): string => {
  const n = v['count'];
  const key = typeof n === 'number' && S[`${k}.${n === 1 ? 'one' : 'other'}`] ? `${k}.${n === 1 ? 'one' : 'other'}` : k;
  return (S[key] ?? k).replace(/\{(\w+)\}/g, (_m, name: string) => String(v[name] ?? `{${name}}`));
};
const NOW = new Date(2026, 10, 17, 22, 40).getTime();
const at = new Date(2026, 10, 17, 22, 31).getTime();
const item = (o: Partial<Item>): Item => ({ id: 'a1', kind: 'file', name: 'x', title: 'x', size: 0, at, by: 'you', ...o }) as Item;

test('a_card_previews_the_thing_itself_as_v1_did', () => {
  expect(thumbOf(item({ kind: 'image', name: 'sunset.png' }))).toEqual({ kind: 'image', src: '/api/stuff/a1/file?r=1' });
  expect(thumbOf(item({ kind: 'doc', name: 'notes.md', excerpt: '# Notes\n\nhi' }))).toEqual({ kind: 'doc', markdown: '# Notes\n\nhi' });
  expect(thumbOf(item({ kind: 'code', name: 'scores.csv', excerpt: 'a,b' }))).toEqual({ kind: 'code', text: 'a,b', lang: 'csv' });
  expect(thumbOf(item({ kind: 'link', name: 'github.com', url: 'https://github.com/WispAgents/wisp' }))).toEqual({ kind: 'link', site: 'github.com', path: '/WispAgents/wisp' });
  expect(thumbOf(item({ kind: 'file', name: 'bundle.zip' }))).toEqual({ kind: 'ext', label: 'ZIP' });
  expect(thumbOf(item({ kind: 'app', name: 'Hello pane', title: 'Hello pane', excerpt: 'counts clicks' }))).toEqual({ kind: 'app', name: 'Hello pane', cover: undefined, initial: 'H', excerpt: 'counts clicks' });
  expect(thumbOf(item({ kind: 'image', pending: true }))).toEqual({ kind: 'pending', icon: 'image' });
});

test('a_cards_facts_line_says_picture_and_its_time_today_is_the_hour', () => {
  const look = cardLook(item({ kind: 'image', name: 'sunset.png', title: 'sunset', size: 81415 }), t, NOW);
  expect(look.facts).toBe('Picture · 80 KB');
  expect(look.label).toBe('Picture: sunset');
  // today reads as the hour (v1): 22:31 on a 24-hour locale, 10:31 PM on en-US (CI's)
  expect(look.time.label).toMatch(/22:31|10:31/);
  expect(look.version).toBeUndefined();
  expect(cardLook(item({ kind: 'doc', name: 'r.md', size: 151, rev: 3, updatedAt: at, prev: { at } }), t, NOW).version?.label).toBe('v3');
});

test('a_published_app_shows_public_and_its_data_warning', () => {
  const look = cardLook(item({ kind: 'app', published: { url: 'https://grid.trywisp.dev/p/x/', rev: 1 }, db: true, rev: 2 }), t, NOW);
  expect(look.pub).toMatchObject({ label: 'Public', old: true });
  expect(look.openData).toContain('Anyone with the link');
  expect(launchLook(item({ kind: 'app' }), t)).toMatchObject({ label: 'Launch', running: false });
  expect(launchLook(item({ kind: 'app', running: { port: 47332 } }), t)).toMatchObject({ label: 'Running', running: true, dot: true });
  expect(launchLook(item({ kind: 'app', running: { port: 0 } }), t)).toMatchObject({ label: 'Starting…', dot: false });
});

test('the_head_says_how_many_things_and_how_much_they_weigh', () => {
  expect(headCount([item({ size: 81415 }), item({ size: 151 })], t, fmtSize)).toBe('2 things · 80 KB');
  expect(headCount([item({ kind: 'link' })], t, fmtSize)).toBe('1 thing');
  expect(headCount([], t, fmtSize)).toBe('');
});

test('nothing_matching_names_the_words_the_kind_and_starred', () => {
  expect(t('stuff.page.none', { what: noneWhat('zzzz', 'all', false, t) })).toBe('Nothing in My Stuff matches “zzzz”.');
  expect(noneWhat('', 'image', true, t)).toBe('pictures, starred');
});

test('the_menu_and_chips_use_v1s_words', () => {
  expect(S['stuff.kind.image.many']).toBe('Pictures');
  expect(S['stuff.act.mention']).toBe('Use in a new task');
  expect(S['stuff.act.edit']).toBe('Edit details…');
  expect(S['stuff.act.revoke']).toBe('Take back access');
  expect(S['stuff.act.task']).toBe('Open the task');
  expect(S['stuff.act.unpinTask']).toBe('Unpin from beside tasks');
});
