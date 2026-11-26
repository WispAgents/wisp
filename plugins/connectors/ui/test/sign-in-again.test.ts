// A signed-in connector whose sign-in ran out (the server refused its token and no refresh helped) says it needs a
// sign-in and its menu offers Sign in again beside Sign out (it offered only Sign out). Signing in again with the kept
// client stamps the sign-in's time anew, so the row stops saying "Signing in…" at once.
import { expect, test } from 'vitest';
import { signItems, statusOf, stillSigning } from '../src/lib.ts';
import type { ConnectorView } from '../src/types.ts';

const view = (over: Partial<ConnectorView>): ConnectorView => ({ id: 'linear', name: 'linear', transport: 'remote', ...over }) as ConnectorView;
const signedIn = { host: 'mcp.linear.app', at: 1_000 };

test('a_sign_in_that_ran_out_says_so_and_offers_sign_in_again', () => {
  const ranOut = view({ signed_in: signedIn, test: { at: 2_000, ok: false, error: 'needs sign-in: invalid_grant', auth: true } } as Partial<ConnectorView>);
  expect(statusOf(ranOut, false).kind).toBe('signin');
  expect(signItems(ranOut)).toEqual(['signin', 'signout']);
  expect(signItems(view({ signed_in: signedIn, test: { at: 2_000, ok: true, count: 1 } } as Partial<ConnectorView>))).toEqual(['signout']);
  expect(signItems(view({}))).toEqual(['signin']);
  expect(signItems(view({ transport: 'local' }))).toEqual([]);
});

test('signing_in_again_is_over_once_the_row_has_the_new_stamp', () => {
  const since = 50_000;
  expect(stillSigning(view({ signed_in: signedIn }), since, since + 1_000)).toBe(true);
  expect(stillSigning(view({ signed_in: { host: 'mcp.linear.app', at: since + 800 } }), since, since + 1_000)).toBe(false);
});
