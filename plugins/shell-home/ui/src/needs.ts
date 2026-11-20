// Needs you on Home (v1 split.js paintHome): the tasks waiting on you, asking ones first then newest, five shown
// with "5 of 12" when there are more. Home doubles as the inbox. Pure, so tests pin the rules.

import type { TaskRow } from './types.ts';
import './i18n/lazy.ts';

export const NEEDS_SHOWN = 5;

/** Waiting on you: asking, wanting approval, a reminder that came due, or finished and not yet seen to. */
export function needsYou(r: TaskRow): boolean {
  if (r.archived || r.forLater) return false;
  if (r.needs === true) return true;
  if (asking(r) || r.reminder) return true;
  return r.state === 'review' || r.state === 'error';
}

/** Asking you something: v1's `waiting` state (a `runtime.ask` comes with an empty `question`), a question or an approval. */
export const asking = (r: TaskRow): boolean => r.state === 'waiting' || !!(r.question || r.approval);
const asks = (r: TaskRow): number => Number(asking(r));

/** v1 feed.js `groups().needs`, as the sidebar sorts it: the ones asking you first, then newest. */
export function needsList(rows: readonly TaskRow[]): TaskRow[] {
  return rows.filter(needsYou).sort((a, b) => asks(b) - asks(a) || b.updatedAt - a.updatedAt || a.id.localeCompare(b.id));
}

/** The word in the row's state column, as a catalogue key. */
export function stateKey(r: TaskRow): string {
  if (r.approval) return 'shell-home.state.approval';
  if (asking(r)) return 'shell-home.state.asking';
  if (r.reminder) return 'shell-home.state.reminder';
  if (r.state === 'error') return 'shell-home.state.error';
  if (r.status === 'blocked') return 'shell-home.state.blocked';
  // a run you stopped (a factory's): Stopped, as its band says (audit G15)
  if (r.status === 'stopped') return 'shell-home.state.stopped';
  return r.state === 'done' ? 'shell-home.state.done' : 'shell-home.state.ready';
}

/** The pill's tone for that word. */
export function stateTone(r: TaskRow): 'accent' | 'warn' | 'bad' | 'good' | 'neutral' {
  if (asking(r)) return 'accent';
  if (r.reminder) return 'warn';
  if (r.state === 'error' || r.status === 'blocked') return 'bad';
  return 'good';
}
