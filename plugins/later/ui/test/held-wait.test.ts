// QA r1 (lead): Later's held actions sent `start` on the wire for every action but Skip, Wait included: wired up, Wait
// would have started a held turn (a changed harness, a plan's limit) without a yes. No command here does that.
import { expect, test } from 'vitest';
import * as later from '../src/commands.ts';

type Wired = { readonly id?: string; readonly wire?: (a: unknown) => unknown };
const wait = { key: 'local:s1', sessionId: 'local:s1', id: 'local:s1', turnId: 't1', action: 'wait' };

test('no_command_sends_start_for_a_wait', () => {
  const all = (Object.values(later) as unknown[]).filter((c): c is Wired => typeof c === 'object' && c !== null && 'wire' in c && typeof (c as Wired).wire === 'function');
  expect(all.length).toBeGreaterThan(0);
  for (const c of all) {
    let sent: unknown;
    try {
      sent = c.wire?.(wait);
    } catch {
      continue;
    }
    const action = typeof sent === 'object' && sent !== null ? (sent as { action?: unknown }).action : undefined;
    expect(action === 'start' && /^runtime\.(held|release)$/.test(c.id ?? ''), `${c.id} sends start for a wait`).toBe(false);
  }
});
