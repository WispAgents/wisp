// Feedback for the wisp team (v1 features/feedback.js): `/feedback …` or `/bug …` in any prompt box sends at once
// (no words: the dialog opens), the dialog (⌘K Send feedback…, Home's foot) sends an idea or a bug, and thumbs on a
// turn (rateTurn). It goes with the harness and model, never the prompt or the code. Sending it is feedback-send.ts, its
// own chunk (budget, ADR-0011): the first page only knows a /feedback when it sees one.
import type { ShellContext } from '../contract.ts';

export const FEEDBACK_CMD = /^\/(feedback|bug)(?:\s|$)/i;

/** The text is `/feedback …` or `/bug …`. */
export const isFeedback = (text: string): boolean => FEEDBACK_CMD.test(text.trim());

export interface FeedbackParams {
  readonly text?: string;
  readonly kind?: 'bug' | 'idea';
  readonly sessionId?: string;
  readonly source?: 'command' | 'dialog';
}

export function parseFeedback(p: unknown): FeedbackParams {
  const o = (typeof p === 'object' && p !== null ? p : {}) as Record<string, unknown>;
  return {
    ...(typeof o['text'] === 'string' && { text: o['text'] }),
    ...(o['kind'] === 'bug' && { kind: 'bug' as const }),
    ...(typeof o['sessionId'] === 'string' && { sessionId: o['sessionId'] }),
    ...(o['source'] === 'command' && { source: 'command' as const }),
  };
}

/** Opens Send feedback (⌘K, Home's foot, `/feedback` with no words). */
export function openFeedback(shell: ShellContext, params: { readonly kind?: 'bug' | 'idea'; readonly text?: string; readonly sessionId?: string; readonly source?: 'command' | 'dialog' } = {}): void {
  shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.feedback' as never, params } });
}

export type { FeedbackArgs } from './feedback-send.ts';
