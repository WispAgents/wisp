// QA r2 phone #13: on the phone apps every My Stuff tile was a grey box with the kind's glyph, even the "sunset"
// picture: the phone screen got an icon only. A picture shows its file and an app its cover, as the page's cards do
// (thumbOf); a still-arriving picture and every other kind keep their glyph.
import { expect, test } from 'vitest';
import { phoneImage } from '../src/card.ts';
import type { Item } from '../src/types.ts';

const item = (o: Partial<Item>): Item => ({ id: 'x1', kind: 'doc', name: 'n', title: 't', at: 1, rev: 2, ...o }) as Item;

test('a_picture_or_an_apps_cover_is_its_phone_thumbnail', () => {
  expect(phoneImage(item({ id: 'sun', kind: 'image', name: 'sunset.jpg' }))).toBe('/api/stuff/sun/file?r=2');
  expect(phoneImage(item({ id: 'ap', kind: 'app', cover: true } as Partial<Item>))).toBe('/api/stuff/ap/cover?r=2');
  expect(phoneImage(item({ id: 'ap', kind: 'app' }))).toBeUndefined();
  expect(phoneImage(item({ kind: 'image', pending: true } as Partial<Item>))).toBeUndefined();
  expect(phoneImage(item({ kind: 'doc' }))).toBeUndefined();
});
