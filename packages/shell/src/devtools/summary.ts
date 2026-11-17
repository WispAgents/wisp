// The inspector's data, pure (plan §4.6.8): one line per devtools entry, newest first, and the
// bug-report JSON with the peers' states added. Entries are summaries already (no row contents,
// no secrets): this only formats them.

import type { DevEntry } from '@wisp/client';

/** The kinds counted, as the client keeps them (`machines_peer`, `stuff.item`: QA r1 #11 counted `machine` and `stuff`,
 *  which no row is). Rows this window has loaded: turns and events come with the tasks that are open. */
export const STORE_KINDS = ['session', 'turn', 'event', 'setting', 'crew_member', 'machines_peer', 'stuff.item'] as const;

/** A computer's line: its version, its build only when it's something else, and how many plugins. */
export function peerLine(info: { readonly version?: string; readonly build?: string; readonly plugins?: readonly unknown[] }, plugins: (n: number) => string): string {
  const v = info.version ?? '';
  const b = info.build && info.build !== v ? info.build : '';
  return [v, b, plugins((info.plugins ?? []).length)].filter(Boolean).join(' · ');
}

export function describe(e: DevEntry): string {
  switch (e.kind) {
    case 'frame':
      return `${e.peer} ← ${e.t}${e.seq === undefined ? '' : ` @${e.seq}`}${e.ops === undefined ? '' : ` (${e.ops} ops)`}`;
    case 'command':
      return `${e.cmd} → ${e.peer} [${e.mid.slice(-8)}]`;
    case 'sent':
      return `sent [${e.mid.slice(-8)}] → ${e.peer}`;
    case 'ack':
      return `ack [${e.mid.slice(-8)}] @${e.seq}`;
    case 'reject':
      return `reject [${e.mid.slice(-8)}] ${e.code}: ${e.message}`;
    case 'transition':
      return `${e.machine}: ${e.from} —${e.via}→ ${e.to}`;
    case 'warn':
      return `warn: ${e.message}`;
  }
}

/** Newest first, each with a key unique within this read (entries carry no id of their own). */
export function newestFirst<T extends { at: number }>(entries: readonly T[], limit = 500): (T & { key: string })[] {
  const out = entries.slice(-limit).map((e, i) => ({ ...e, key: `${e.at}:${i}` }));
  return out.reverse();
}
