// The owner (2026-11-27): a task you clicked Ship on leaves Needs you for its own Shipping section, between Needs you
// and Working, while wisp.ship pushes it and until its PR lands.
import { expect, test } from 'vitest';
import { flatten, itemKey } from '../src/model/flat.ts';
import { SECTION_IDS, sections, type Keyed } from '../src/model/sections.ts';
import { signature } from '../src/model/placement.ts';
import type { EntityKey, PrRef, SessionRow } from '../src/types.ts';

const row = (id: string, o: Partial<SessionRow> = {}): Keyed => ({ key: `local:${id}` as EntityKey, s: { id, title: id, agent: 'claude', state: 'review', createdAt: 1, updatedAt: 1, ...o } as SessionRow });
const pr = (state: PrRef['state']): PrRef => ({ number: 7, url: 'https://github.com/o/r/pull/7', state });

test('shipping_sits_between_needs_you_and_working', () => {
  expect(SECTION_IDS.slice(SECTION_IDS.indexOf('needs'), SECTION_IDS.indexOf('working') + 1)).toEqual(['needs', 'shipping', 'working']);
});

test('a_shipping_or_open_pr_task_leaves_needs_you_for_shipping', () => {
  const all = [
    row('plain'),
    row('pushing', { shipping: { step: 'push' }, updatedAt: 2 }),
    row('open', { pr: pr('OPEN'), unread: true, updatedAt: 3 }),
    row('merged', { pr: pr('MERGED') }),
    row('fixing', { state: 'running', pr: pr('OPEN') }),
    row('asks', { state: 'waiting', pr: pr('OPEN') }),
    row('done', { state: 'done', pr: pr('OPEN') }),
    row('gone', { archived: true, pr: pr('OPEN') }),
  ];
  const sec = sections(all);
  expect(sec.shipping.map((e) => e.key)).toEqual(['local:open', 'local:pushing']);
  // a merged PR is back for you to look at; one running, asking, done or archived sits where that says
  expect(sec.needs.map((e) => e.key)).toEqual(['local:asks', 'local:plain', 'local:merged']);
  expect(sec.working.map((e) => e.key)).toEqual(['local:fixing']);
  expect(sec.done.map((e) => e.key)).toEqual(['local:done']);
  expect(sec.archived.map((e) => e.key)).toEqual(['local:gone']);
  expect(flatten(sec, { toggled: new Set(), split: false }).map(itemKey)).toEqual([
    'h:needs', 'local:asks', 'local:plain', 'local:merged', 'h:shipping', 'local:open', 'local:pushing', 'h:working', 'local:fixing',
    'h:done', 'h:archived',
  ]);
});

// the list rebuilds its sections only when a row's placement signature changes: Ship and the PR landing must change it
test('shipping_and_the_pr_landing_change_the_rows_placement', () => {
  const s = row('a').s;
  expect(signature({ ...s, shipping: { step: 'commit' } })).not.toBe(signature(s));
  expect(signature({ ...s, pr: pr('OPEN') })).not.toBe(signature({ ...s, pr: pr('MERGED') }));
});
