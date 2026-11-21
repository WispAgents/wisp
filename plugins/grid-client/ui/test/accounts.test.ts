import { expect, test } from 'vitest';
import { accountsOf, hostOf, removed, switched } from '../src/accounts.ts';
import type { CloudRow } from '../src/commands.ts';

const acct = (grid: string, handle: string, current = false) => ({ key: `${grid}#${handle}`, grid, handle, machine: `m-${handle}`, current });
const row = (accounts: readonly ReturnType<typeof acct>[]): CloudRow => {
  const c = accounts?.find((a) => a.current);
  return {
    signedIn: !!c,
    grid: c?.grid ?? '',
    handle: c?.handle ?? '',
    machine: c?.machine ?? '',
    computer: { on: true, state: 'online', label: 'studio', url: 'https://studio--ada.g1', error: '' },
    signin: null,
    accounts,
  };
};
const G1 = 'https://g1.dev';
const G2 = 'https://g2.dev';

test('each_account_is_listed_and_an_older_servers_row_still_shows_its_one', () => {
  const r = row([acct(G1, 'ada'), acct(G2, 'ada'), acct(G1, 'bea', true)]);
  expect(accountsOf(r).map((a) => [a.handle, hostOf(a.grid), a.current])).toEqual([
    ['ada', 'g1.dev', false],
    ['ada', 'g2.dev', false],
    ['bea', 'g1.dev', true],
  ]);
  expect(accountsOf({ signedIn: true, grid: G1, handle: 'ada', machine: 'm1' })).toEqual([{ ...acct(G1, 'ada', true), machine: 'm1' }]);
  expect(accountsOf({ signedIn: false, grid: '', handle: '', machine: '', accounts: null })).toEqual([]);
  expect(accountsOf({ signedIn: true, grid: G1, handle: 'ada', machine: '', accounts: [{ nope: 1 } as never, acct(G2, 'bo', true)] }).map((a) => a.handle)).toEqual(['bo']);
});

test('switch_marks_the_other_current_at_once_and_the_tunnel_waits_for_the_server', () => {
  const r = row([acct(G1, 'ada', true), acct(G2, 'bo')]);
  const p = switched(r, `${G2}#bo`);
  expect([p.grid, p.handle, p.machine, p.signedIn]).toEqual([G2, 'bo', 'm-bo', true]);
  expect(p.accounts?.map((a) => a.current)).toEqual([false, true]);
  expect(p.computer).toMatchObject({ on: false, state: 'off' });
  expect(switched(r, `${G1}#ada`).computer).toEqual(r.computer);
  expect(switched(r, 'https://gone#x')).toEqual({});
});

test('sign_out_of_one_takes_only_it_and_the_first_left_becomes_current', () => {
  const r = row([acct(G1, 'ada'), acct(G2, 'bo', true), acct(G1, 'bea')]);
  const notCurrent = removed(r, `${G1}#bea`);
  expect(notCurrent.accounts?.map((a) => [a.handle, a.current])).toEqual([['ada', false], ['bo', true]]);
  expect(notCurrent.handle).toBe('bo');
  const current = removed(r, `${G2}#bo`);
  expect(current.accounts?.map((a) => [a.handle, a.current])).toEqual([['ada', true], ['bea', false]]);
  expect([current.grid, current.handle]).toEqual([G1, 'ada']);
  const last = removed(row([acct(G1, 'ada', true)]), `${G1}#ada`);
  expect([last.signedIn, last.handle, last.accounts]).toEqual([false, '', []]);
  expect(removed(r, 'https://gone#x')).toEqual({});
});
