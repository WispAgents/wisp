// The inspector's data, pure (plan §4.6.8): one line per devtools entry, newest first, and the
// bug-report JSON with the peers' states added. Entries are summaries already (no row contents,
// no secrets): this only formats them.

import type { DevEntry } from '@wisp/client';
import '../i18n/lazy.ts';

/** The kinds counted, as the client keeps them (`machines_peer`, `stuff.item`: QA r1 #11 counted `machine` and `stuff`,
 *  which no row is). Rows this window has loaded: turns and events come with the tasks that are open. */
export const STORE_KINDS = ['session', 'turn', 'event', 'setting', 'crew_member', 'machines_peer', 'stuff.item'] as const;

/** Rows that load with the task that's open (its turns, its events), not with the window. */
const PER_TASK: ReadonlySet<string> = new Set(['turn', 'event']);

/** A kind's count, as this window holds it; per-task rows with none loaded say they come with an open task, not "0
 *  rows" (QA desktop r2 #11 read that as a broken store). */
export function rowsLine(kind: string, count: number, t: (key: string, vars?: Readonly<Record<string, string | number>>) => string): string {
  return count === 0 && PER_TASK.has(kind) ? t('shell.diag.perTask') : t('shell.diag.rows', { count });
}

/** A computer's line: its version, its build only when it's something else, and how many plugins. */
export function peerLine(info: { readonly version?: string; readonly build?: string; readonly plugins?: readonly unknown[] }, plugins: (n: number) => string): string {
  const v = info.version ?? '';
  const b = info.build && info.build !== v ? info.build : '';
  return [v, b, plugins((info.plugins ?? []).length)].filter(Boolean).join(' · ');
}

/** A computer's name in Diagnostics (setup audit B9: the cloud computer read "this computer", as every wisp calls
 *  itself). This wisp is its own name; another is the name its machine row gives it (`machines_peer`, keyed
 *  `<home>:<machine>`, the machine being its peer id), else its own unless that's the generic "this computer", else
 *  its id. */
export function peerTitle(
  peer: string,
  home: string,
  own: string,
  machines: readonly { readonly id: string | number; readonly body?: { readonly name?: string | null } | undefined }[],
): string {
  if (peer === home) return own;
  const row = machines.find((m) => {
    const id = String(m.id);
    return id.slice(id.indexOf(':') + 1) === peer;
  });
  const named = row?.body?.name;
  if (named) return named;
  if (row) return peer;
  return own && own !== 'this computer' ? own : peer;
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
