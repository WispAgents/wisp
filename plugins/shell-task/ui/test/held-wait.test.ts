// QA r1 (lead): the task page's unused releaseHeld sent `start` for every action but Cancel, Wait included: wired up,
// Wait would have started a held turn without a yes. No command here does that (the card's Start / Skip are TurnFoot's).
import { expect, test } from 'vitest';
import * as later from '../src/commands/session.ts';

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
