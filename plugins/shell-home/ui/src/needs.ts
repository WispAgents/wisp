// Needs you on Home (v1 split.js paintHome): the tasks waiting on you, asking ones first then newest, five shown
// with "5 of 12" when there are more. Home doubles as the inbox. Pure, so tests pin the rules.

import type { TaskRow } from './types.ts';
import './i18n/lazy.ts';

export const NEEDS_SHOWN = 5;

/** Waiting on you: asking, wanting approval, a reminder that came due, or finished and not yet seen to. */
export function needsYou(r: TaskRow): boolean {
  if (r.archived || r.forLater) return false;
  if (r.needs === true) return true;
  if (r.question || r.approval || r.reminder) return true;
  return r.state === 'review' || r.state === 'error';
}

/** Asking you something (a question or an approval): v1's `waiting` state, first in Needs you. */
const asks = (r: TaskRow): number => Number(!!(r.question || r.approval));

/** v1 feed.js `groups().needs`, as the sidebar sorts it: the ones asking you first, then newest. */
export function needsList(rows: readonly TaskRow[]): TaskRow[] {
  return rows.filter(needsYou).sort((a, b) => asks(b) - asks(a) || b.updatedAt - a.updatedAt || a.id.localeCompare(b.id));
}

/** The word in the row's state column, as a catalogue key. */
export function stateKey(r: TaskRow): string {
  if (r.question) return 'shell-home.state.asking';
  if (r.approval) return 'shell-home.state.approval';
  if (r.reminder) return 'shell-home.state.reminder';
  if (r.state === 'error') return 'shell-home.state.error';
  if (r.status === 'blocked') return 'shell-home.state.blocked';
  return r.state === 'done' ? 'shell-home.state.done' : 'shell-home.state.ready';
}

/** The pill's tone for that word. */
export function stateTone(r: TaskRow): 'accent' | 'warn' | 'bad' | 'good' | 'neutral' {
  if (r.question || r.approval) return 'accent';
  if (r.reminder) return 'warn';
  if (r.state === 'error' || r.status === 'blocked') return 'bad';
  return 'good';
}
