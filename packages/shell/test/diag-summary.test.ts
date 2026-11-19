// QA r1 #11: Settings → Diagnostics read "turn 0, event 0, machine 0, stuff 0 rows" and "2.0.0 · 2.0.0 · 50 plugins".
// Two kinds were named wrong (the client keeps machines as `machines_peer`, My Stuff as `stuff.item`), and the build
// was shown even when it's the version.
import { expect, test } from 'vitest';
import { STORE_KINDS, peerLine, peerTitle, rowsLine } from '../src/devtools/summary.ts';

test('diagnostics_counts_the_kinds_the_client_keeps_and_says_the_version_once', () => {
  expect(STORE_KINDS).toContain('machines_peer');
  expect(STORE_KINDS).toContain('stuff.item');
  expect(STORE_KINDS).not.toContain('machine');
  expect(STORE_KINDS).not.toContain('stuff');
  expect(peerLine({ version: '2.0.0', build: '2.0.0', plugins: Array(50).fill('p') }, (n) => `${n} plugins`)).toBe('2.0.0 · 50 plugins');
  expect(peerLine({ version: '2.0.0', build: 'a1b2c3d', plugins: ['p'] }, (n) => `${n} plugins`)).toBe('2.0.0 · a1b2c3d · 1 plugins');
});

// QA desktop r2 #11: "turn 0 rows, event 0 rows" read as a broken store: those rows load per open task. With none
// loaded they say so; the section says what its numbers count.
test('per_task_rows_say_they_load_with_an_open_task_instead_of_zero', () => {
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => (v ? `${k}:${JSON.stringify(v)}` : k);
  expect(rowsLine('turn', 0, t)).toBe('shell.diag.perTask');
  expect(rowsLine('event', 0, t)).toBe('shell.diag.perTask');
  expect(rowsLine('turn', 12, t)).toBe('shell.diag.rows:{"count":12}');
  expect(rowsLine('session', 0, t)).toBe('shell.diag.rows:{"count":0}');
});

// Setup audit B9: signed in with the cloud computer awake, Computers listed "this computer" twice: every wisp calls
// itself that, and the second is the cloud computer. Another computer is named as this wisp knows it (its machine row).
test('another_computer_is_named_by_its_machine_row_never_this_computer_setup_audit_b9', () => {
  const machines = [{ id: 'local:qa-alice', body: { name: 'qa-alice (cloud)' } }, { id: 'local:studio', body: {} }];
  expect(peerTitle('local', 'local', 'this computer', machines)).toBe('this computer');
  expect(peerTitle('qa-alice', 'local', 'this computer', machines)).toBe('qa-alice (cloud)');
  expect(peerTitle('studio', 'local', 'this computer', machines)).toBe('studio');
  expect(peerTitle('laptop', 'local', 'Ada’s laptop', [])).toBe('Ada’s laptop');
  expect(peerTitle('laptop', 'local', 'this computer', [])).toBe('laptop');
});
