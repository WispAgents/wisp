// Audit B #3: "vs Compare" opened with the run's entity key (`local:<id>`) while the store's rows carry bare ids, so
// the group was empty ("These runs are gone."). compareGroup resolves bare and keyed ids alike, and hands back rows
// keyed, as navigation and commands need them.
import { expect, test } from 'vitest';
import { compareGroup } from '../src/frame/compare-group.ts';

const row = (id: string, groupId: string | undefined, at: number, archived = false) => ({ id, created_at: at, archived, summary: groupId ? { groupId } : {} });

test('a_compare_opened_with_a_keyed_id_finds_a_group_of_bare_rows_and_keys_them', () => {
  const rows = [row('a1', 'g1', 1), row('b2', 'g1', 2), row('c3', 'g2', 3), row('d4', 'g1', 4, true)];
  const keys = ['local:a1', 'local:b2', 'local:c3', 'local:d4'];
  const g = compareGroup(rows, keys, 'local:b2');
  expect(g.map((r) => r.id)).toEqual(['local:a1', 'local:b2']);
  // the bare id too, and rows that are already keyed (the mock's)
  expect(compareGroup(rows, keys, 'a1').map((r) => r.id)).toEqual(['local:a1', 'local:b2']);
  const keyed = [row('local:a1', 'g1', 1), row('local:b2', 'g1', 2)];
  expect(compareGroup(keyed, ['local:a1', 'local:b2'], 'local:a1').map((r) => r.id)).toEqual(['local:a1', 'local:b2']);
  // no group, or no such run: empty (the dialog says the runs are gone)
  expect(compareGroup(rows, keys, 'local:zz')).toEqual([]);
  expect(compareGroup([row('e5', undefined, 5)], ['local:e5'], 'local:e5')).toEqual([]);
});
