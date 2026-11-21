// ⋯ Mark unread on an open task (v2's own: v1 had none on the page). Staying would read it again, so the page leaves
// for the next task that needs you (else home) and says so, with Undo (audit B #5: it jumped with no word, and the next
// ⋯ action was taken for the old task's). Pure: TaskView navigates, toasts and sends.
import { nextNeedsYou } from './rows-view.ts';
import type { SessionRow } from '../rows.ts';

export type Leave = { readonly t: 'openTask'; readonly id: string } | { readonly t: 'home' };

export function unreadLeave(rows: readonly SessionRow[], key: string, title: string, peer: string): { readonly to: Leave; readonly toast: { readonly key: string; readonly vars: { readonly title: string } } } {
  const next = nextNeedsYou(rows, key);
  const to: Leave = next ? { t: 'openTask', id: next.includes(':') ? next : `${peer}:${next}` } : { t: 'home' };
  return { to, toast: { key: 'shell.toast.unread', vars: { title } } };
}
