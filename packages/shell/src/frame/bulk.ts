// The phone list's bulk actions (ADR-0065 step 13, design/mobile.md §A.4 Selection mode): Mark done, Save for later,
// Archive and Delete on every picked task, as each row's own menu does them (the same shell commands as E, A, Delete
// and the task page's Save for later: optimistic, a refusal rolls back with the client's toast). A task that can't
// take an action (still working, archived, already done) is left as it is; an action none of the picked tasks can take
// is disabled. Which tasks take which action is pure (`bulkTargets`); `runBulk` sends them.
import type { Client } from '@wisp/client';
import type { Toaster } from '@wisp/ui';
import { SESSION, type SessionRow } from '../rows.ts';
import { archive, forLater, isSaved, remove, setState } from './commands.ts';

export type BulkAction = 'done' | 'later' | 'archive' | 'delete';
export const BULK_ACTIONS: readonly BulkAction[] = ['done', 'later', 'archive', 'delete'];

const busy = (s: SessionRow): boolean => s.state === 'running' || s.state === 'waiting';

/** Can this task take the action (as its row's menu would offer it). */
export function canBulk(action: BulkAction, s: SessionRow): boolean {
  if (action === 'delete') return true;
  if (action === 'archive') return !s.archived && !busy(s);
  if (action === 'done') return !s.archived && !busy(s) && s.state !== 'done';
  // as the row's menu: finished, not archived, not waiting to send (`pending`), not saved already
  return !s.archived && s.summary['pending'] !== true && s.body?.['pending'] !== true && (s.state === 'review' || s.state === 'done') && !isSaved(s.summary['forLater']);
}

/** The picked tasks that take the action, in the order picked (gone ones left out). */
export function bulkTargets(action: BulkAction, keys: readonly string[], row: (key: string) => SessionRow | undefined): string[] {
  return keys.filter((k) => {
    const s = row(k);
    return s !== undefined && canBulk(action, s);
  });
}

export interface BulkCtx {
  readonly client: Client;
  readonly toast: Toaster;
  readonly t: (key: string, vars?: Readonly<Record<string, string | number>>) => string;
}

const quiet = (p: Promise<unknown>): void => {
  p.catch(() => undefined);
};

/** Sends the action for each target at once (each row changes in the same frame) and says how many, with Undo where
 *  the row's own action has it (Delete is asked first, so it has none). Returns how many were sent. */
export function runBulk(c: BulkCtx, action: BulkAction, keys: readonly string[]): number {
  const row = (k: string): SessionRow | undefined => c.client.entities.peek<SessionRow>(SESSION, k as never) as SessionRow | undefined;
  const targets = bulkTargets(action, keys, row);
  if (!targets.length) return 0;
  const run = c.client.commands.run;
  const n = targets.length;
  if (action === 'done') {
    const was = new Map(targets.map((k) => [k, row(k)?.state ?? 'review']));
    for (const id of targets) {
      quiet(run(setState, { id, state: 'done' }));
      c.client.nav.navigate({ t: 'markDone', id: id as never });
    }
    c.toast.success(c.t('shell.select.toast.done', { count: n }), { action: { label: c.t('shell.select.undo'), onClick: () => targets.forEach((id) => quiet(run(setState, { id, state: (was.get(id) ?? 'review') as never }))) } });
  } else if (action === 'archive') {
    for (const id of targets) {
      quiet(run(archive, { id, archived: true }));
      c.client.nav.navigate({ t: 'archive', id: id as never });
    }
    c.toast.show(c.t('shell.select.toast.archived', { count: n }), { action: { label: c.t('shell.select.undo'), onClick: () => targets.forEach((id) => quiet(run(archive, { id, archived: false }))) } });
  } else if (action === 'later') {
    for (const id of targets) quiet(run(forLater, { id, forLater: true }));
    c.toast.show(c.t('shell.select.toast.later', { count: n }), { action: { label: c.t('shell.select.undo'), onClick: () => targets.forEach((id) => quiet(run(forLater, { id, forLater: false }))) } });
  } else {
    for (const id of targets) {
      quiet(run(remove, { id }));
      c.client.nav.navigate({ t: 'delete', id: id as never });
    }
    c.toast.show(c.t('shell.select.toast.deleted', { count: n }));
  }
  return n;
}
