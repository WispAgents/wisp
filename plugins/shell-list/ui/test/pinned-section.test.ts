// QA round 3 desktop #11, the owner's delegate (2026-11-18): Pin only added a pill and the row stayed where it was.
// Pinned tasks lead the list in a Pinned section at the top, folded and unfolded like the others.
import { expect, test } from 'vitest';
import { flatten, isOpenSection, itemKey } from '../src/model/flat.ts';
import { SECTION_IDS, sections, type Keyed } from '../src/model/sections.ts';
import type { EntityKey, SessionRow } from '../src/types.ts';
import { signature } from '../src/model/placement.ts';

const row = (id: string, o: Partial<SessionRow> = {}): Keyed => ({ key: `local:${id}` as EntityKey, s: { id, title: id, agent: 'claude', state: 'review', createdAt: 1, updatedAt: 1, ...o } as SessionRow });

test('pinned_tasks_lead_the_list_in_their_own_section', () => {
  expect(SECTION_IDS[0]).toBe('pinned');
  const all = [row('a'), row('b', { pinned: true }), row('c', { state: 'done', pinned: true, updatedAt: 9 }), row('d', { state: 'done' }), row('e', { pinned: true, archived: true })];
  const sec = sections(all);
  // pinned ones, latest first, out of Needs you and Done meanwhile; an archived one stays in Archived
  expect(sec.pinned.map((e) => e.key)).toEqual(['local:c', 'local:b']);
  expect(sec.needs.map((e) => e.key)).toEqual(['local:a']);
  expect(sec.done.map((e) => e.key)).toEqual(['local:d']);
  expect(sec.archived.map((e) => e.key)).toEqual(['local:e']);
  // first in the list, open by default, folded by a toggle like any section
  expect(isOpenSection('pinned', new Set())).toBe(true);
  expect(isOpenSection('pinned', new Set(['pinned']))).toBe(false);
  expect(flatten(sec, { toggled: new Set(), split: false }).map(itemKey).slice(0, 3)).toEqual(['h:pinned', 'local:c', 'local:b']);
});

// the list rebuilds its sections only when a row's placement signature changes (model/placement.ts): Pin must change it,
// or the row stayed in Done with its pill (seen on a real server)
test('pinning_changes_the_rows_placement_so_the_list_moves_it', () => {
  const s = row('a').s;
  expect(signature({ ...s, pinned: true } as SessionRow)).not.toBe(signature(s));
});
