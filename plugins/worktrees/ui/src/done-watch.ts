// Which of this client's commands mark a task done: apart from the rest of done.ts (its strings, the dialog's words) so
// the plugin's setup watches for a Done without loading them (ADR-0105).
import type { EntityKey, PendingInfo } from '@wisp/client';

export const SESSION = 'session';

/**
 * The tasks a command of this client just marked done: a pending op (the optimistic patch of
 * `runtime.setState`, from the task page, its row, the list or a shortcut) that sets a session's state to
 * `done`, from a command not seen before. Done from a channel, the phone's notification or an agent is
 * another client's (or none): it never shows here, so it never asks (v1: no dialog from anywhere else).
 */
export function newlyDone(pending: readonly PendingInfo[], seen: Set<string>): EntityKey[] {
  const out: EntityKey[] = [];
  for (const p of pending) {
    if (seen.has(p.mid)) continue;
    seen.add(p.mid);
    for (const op of p.ops) {
      if (op.e !== SESSION || op.op !== 'patch') continue;
      if (op.ops.some((x) => x.op === 'replace' && x.path === '/state' && x.value === 'done') && !out.includes(op.id)) out.push(op.id);
    }
  }
  return out;
}
