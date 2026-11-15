// The phone apps' computer filter (ADR-0065 step 13): the Tasks bar's title narrows the merged list to one computer's
// rows (keys `<peer>:…`); with no computer picked (the browser, All) every row shows, as before.
import { expect, test } from 'vitest';
import type { SessionRow } from '../src/types.ts';
import { flatSession } from '../src/model/wire.ts';
import { sections } from '../src/model/sections.ts';

const row = (id: string, state: string): SessionRow =>
  flatSession({ id, title: id, state, agent: 'claude', created_at: 1, updated_at: 2, body: { cwd: '/w' }, summary: { status: state, preview: '' } } as unknown as SessionRow);

test('the_computer_filter_shows_only_that_computers_rows_in_every_section_and_none_picked_shows_all', () => {
  const keyed = [
    { key: 'mac:a', s: row('a', 'waiting') },
    { key: 'mac:b', s: row('b', 'running') },
    { key: 'studio:c', s: row('c', 'waiting') },
    { key: 'studio-x:d', s: row('d', 'running') },
  ];
  const ids = (computer?: string): string[] =>
    Object.values(sections(keyed as never, { project: 'all', ...(computer !== undefined && { computer }) }))
      .flat()
      .map((e) => (e as { s?: SessionRow }).s?.id ?? '')
      .filter(Boolean)
      .sort();
  expect(ids()).toEqual(['a', 'b', 'c', 'd']);
  expect(ids('mac')).toEqual(['a', 'b']);
  // a peer id that is a prefix of another's is not that one
  expect(ids('studio')).toEqual(['c']);
  // Needs you keeps the picked computer's question only
  const needs = sections(keyed as never, { project: 'all', computer: 'studio' }).needs.map((e) => (e as { s?: SessionRow }).s?.id);
  expect(needs).toEqual(['c']);
});
