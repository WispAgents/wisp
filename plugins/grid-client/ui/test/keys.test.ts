import { expect, test } from 'vitest';
import { adding, keyLine, keysOf, madeOf, nameProblem, restored, settled, tryLine, without } from '../src/keys.ts';
import { t } from '../src/i18n.ts';

const say = t as never;
const ago = (at: number): string => `${(100 - at) / 10}m`;

test('api_keys_list_newest_first_and_never_carry_a_key', () => {
  const v = keysOf({
    keys: [
      { id: 'a', name: 'ci', hint: 'Zq9x', createdAt: 10, lastUsedAt: null, key: 'wgk_never', tokenHash: 'h' },
      { id: 'b', name: 'deploy', hint: 'AbCd', createdAt: 30, lastUsedAt: 90 },
      { nope: true },
    ],
    address: 'https://ada.trywisp.dev',
  });
  expect(v.keys.map((k) => k.id)).toEqual(['b', 'a']);
  expect(JSON.stringify(v)).not.toContain('wgk_');
  expect(JSON.stringify(v)).not.toContain('tokenHash');
  expect(v.address).toBe('https://ada.trywisp.dev');
  expect(keysOf(null)).toEqual({ keys: [], address: '' });
  expect(keyLine(v.keys[0]!, ago, say)).toBe('…AbCd · Created 7m · Last used 1m');
  expect(keyLine(v.keys[1]!, ago, say)).toBe('…Zq9x · Created 9m · Never used');
});

test('a_new_key_shows_at_once_then_takes_the_grids_row_or_goes_on_refusal', () => {
  const list = keysOf({ keys: [{ id: 'a', name: 'ci', hint: 'Zq9x', createdAt: 10, lastUsedAt: null }] }).keys;
  const now = adding(list, 'new-1', '  deploy  ', 50);
  expect(now.map((k) => [k.id, k.name, k.pending ?? false])).toEqual([['new-1', 'deploy', true], ['a', 'ci', false]]);
  expect(keyLine(now[0]!, ago, say)).toBe('Making… · Created 5m · Never used');
  const made = madeOf({ id: 'c', name: 'deploy', hint: 'AbCd', createdAt: 51, lastUsedAt: null, key: 'wgk_xyzAbCd' });
  expect(made?.key).toBe('wgk_xyzAbCd');
  expect(settled(now, 'new-1', made?.row).map((k) => [k.id, k.pending ?? false])).toEqual([['c', false], ['a', false]]);
  expect(settled(now, 'new-1', undefined)).toEqual(list);
  expect(madeOf({ id: 'c', name: 'deploy' })).toBeUndefined();
  expect(madeOf({ id: 'c', name: 'deploy', key: 'sk-other' })).toBeUndefined();
});

test('a_revoke_goes_at_once_and_comes_back_in_place_if_refused', () => {
  const list = keysOf({ keys: [3, 2, 1].map((n) => ({ id: `k${n}`, name: `k${n}`, hint: 'xxxx', createdAt: n, lastUsedAt: null })) }).keys;
  const gone = without(list, 'k2');
  expect(gone.map((k) => k.id)).toEqual(['k3', 'k1']);
  expect(restored(gone, list[1]!).map((k) => k.id)).toEqual(['k3', 'k2', 'k1']);
  expect(restored(list, list[1]!)).toEqual(list);
});

test('a_name_is_needed_and_60_characters_at_most_and_the_try_line_names_no_key', () => {
  expect(nameProblem('  ')).toBe('grid-client.keys.nameNeeded');
  expect(nameProblem('x'.repeat(61))).toBe('grid-client.keys.nameLong');
  expect(nameProblem('🔑'.repeat(60))).toBeUndefined();
  expect(nameProblem(' deploy ')).toBeUndefined();
  expect(tryLine('https://ada.trywisp.dev/')).toBe('curl -H "Authorization: Bearer $WISP_KEY" https://ada.trywisp.dev/api/health');
});
