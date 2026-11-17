// QA r1 #11: Settings → Diagnostics read "turn 0, event 0, machine 0, stuff 0 rows" and "2.0.0 · 2.0.0 · 50 plugins".
// Two kinds were named wrong (the client keeps machines as `machines_peer`, My Stuff as `stuff.item`), and the build
// was shown even when it's the version.
import { expect, test } from 'vitest';
import { STORE_KINDS, peerLine } from '../src/devtools/summary.ts';

test('diagnostics_counts_the_kinds_the_client_keeps_and_says_the_version_once', () => {
  expect(STORE_KINDS).toContain('machines_peer');
  expect(STORE_KINDS).toContain('stuff.item');
  expect(STORE_KINDS).not.toContain('machine');
  expect(STORE_KINDS).not.toContain('stuff');
  expect(peerLine({ version: '2.0.0', build: '2.0.0', plugins: Array(50).fill('p') }, (n) => `${n} plugins`)).toBe('2.0.0 · 50 plugins');
  expect(peerLine({ version: '2.0.0', build: 'a1b2c3d', plugins: ['p'] }, (n) => `${n} plugins`)).toBe('2.0.0 · a1b2c3d · 1 plugins');
});
