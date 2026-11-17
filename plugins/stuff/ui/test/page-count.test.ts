// My Stuff's header count (QA r1 #24: "1 things"): one thing, two things.
import { expect, test } from 'vitest';
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };

test('my_stuff_header_says_one_thing_and_many_things', () => {
  const s = lazy as Record<string, string>;
  expect(s['stuff.page.count']).toBeUndefined();
  expect(s['stuff.page.count.one']).toBe('{count} thing');
  expect(s['stuff.page.count.other']).toBe('{count} things');
});
