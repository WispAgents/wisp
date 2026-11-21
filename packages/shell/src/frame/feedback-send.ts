// Sending feedback to the wisp team (feedback.ts): what `/feedback …` or `/bug …` says, the command that carries it, and
// what the dialog may send. Its own chunk, fetched when a /feedback is typed or the dialog opens (budget, ADR-0011).
import { defineCommand, ownerOf, type PeerId } from '@wisp/client';
import type { ShellContext } from '../contract.ts';
import '../i18n/lazy.ts';
import { FEEDBACK_CMD, openFeedback } from './feedback.ts';

/** What `/feedback …` or `/bug …` says: a bug or an idea, and the words (empty: open the dialog). */
export function feedbackOf(text: string): { readonly kind: 'bug' | 'idea'; readonly note: string } {
  const t = text.trim();
  const m = FEEDBACK_CMD.exec(t);
  return { kind: m?.[1]?.toLowerCase() === 'bug' ? 'bug' : 'idea', note: t.replace(FEEDBACK_CMD, '').trim() };
}

export interface FeedbackArgs {
  readonly peer: PeerId;
  readonly text: string;
  readonly kind?: 'bug' | 'idea';
  readonly rating?: 'up' | 'down';
  readonly sessionId?: string;
  readonly source: 'command' | 'dialog';
}

/** A note to the wisp team, through this wisp (the thanks shows at once; a refusal puts the words back). */
export const sendFeedback = defineCommand<FeedbackArgs, { ok: boolean; sent: boolean; available: boolean }>({
  id: 'telemetry.feedback',
  owner: (a) => a.peer,
  wire: (a) => ({
    text: a.text,
    source: a.source,
    ...(a.kind === 'bug' && { kind: 'bug' }),
    ...(a.kind !== 'bug' && a.rating && { rating: a.rating }),
    ...(a.sessionId && { sessionId: a.sessionId.slice(a.sessionId.indexOf(':') + 1) }),
  }),
  optimistic: 'none',
  pending: 'the dialog closes with a thank-you at once',
});

/** The dialog can send: a bug needs words, an idea words or a thumb. */
export const canSend = (kind: 'bug' | 'idea', rating: 'up' | 'down' | undefined, text: string): boolean => text.trim() !== '' || (kind !== 'bug' && rating !== undefined);

/** `/feedback …` or `/bug …` typed in a prompt box (the box is already empty): sent at once with a thank-you; no
 *  words opens the dialog; a refusal puts the text back (`restore`) and says why. */
export function feedbackCommand(shell: ShellContext, text: string, o: { readonly home: PeerId; readonly sessionId?: string; readonly restore: () => void }): void {
  const { kind, note } = feedbackOf(text);
  if (!note) return openFeedback(shell, { kind, source: 'command', ...(o.sessionId && { sessionId: o.sessionId }) });
  shell.toast.success(shell.t('shell.feedback.thanks'));
  void shell.client.commands
    .run(sendFeedback, { peer: o.sessionId ? ownerOf(o.sessionId) : o.home, text: note, kind, source: 'command', ...(o.sessionId && { sessionId: o.sessionId }) })
    .catch((e: unknown) => {
      o.restore();
      shell.toast.error(e instanceof Error ? e.message : String(e));
    });
}
