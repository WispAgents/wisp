// The open task's keys (v1 kbd.js): the shell's shortcuts name an action, the task page does it, as its button
// or More menu would. A cancelable DOM event, so neither side holds the other: the page cancels what it handled.

export const TASK_KEY = 'wisp-task-key';

export type TaskKey =
  | 'reply'
  | 'later'
  | 'rename'
  | 'openIn'
  | 'shell'
  | 'pr'
  | 'changes'
  | 'transcript'
  | 'find'
  | 'rerun'
  | 'fork'
  | 'ship'
  | 'done'
  | 'archive'
  | 'delete'
  | 'collapse'
  | 'top'
  | 'panel'
  | `answer:${1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9}`;

/** A key pressed on a picked row that needs its task's page (V, T, P, D, S, ⌃`): the list opens the task and the page,
 *  once it listens, does it (audit A #12). One at a time; any task page that opens clears it, and it's good for
 *  PENDING_KEY_MS only (a page that never opened doesn't fire it on a later visit). */
export const PENDING_KEY: { id: string | undefined; act: TaskKey | undefined; at: number } = { id: undefined, act: undefined, at: 0 };
export const PENDING_KEY_MS = 5_000;

/** The key left for task `key`'s page, if it's still good; the slot is cleared either way. */
export function takePendingKey(key: string, now: number): TaskKey | undefined {
  const act = PENDING_KEY.id === key && now - PENDING_KEY.at <= PENDING_KEY_MS ? PENDING_KEY.act : undefined;
  PENDING_KEY.id = undefined;
  PENDING_KEY.act = undefined;
  return act;
}
