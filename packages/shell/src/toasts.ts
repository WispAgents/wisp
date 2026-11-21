// Toasts the peers ask for and the ones a refused command needs (plan §4.6.5): a reject rolls the
// optimistic patch back (the client does that) and says why here. A notice can carry where its
// Open goes; Open navigates with openFromToast, which closes whatever sits over the target (#573).

import type { CommandError, Scope, Target } from '@wisp/client';
import type { Toaster } from '@wisp/ui';
import type { ShellContext } from './contract.ts';

interface ToastNotice {
  message?: unknown;
  tone?: unknown;
  description?: unknown;
  target?: unknown;
}

const isTarget = (v: unknown): v is Target => typeof v === 'object' && v !== null && typeof (v as { to?: unknown }).to === 'string';

/** Whether a prompt box answers a refusal reason in place (a hold row, `ShellContext.sendHolds`): no toast then. The
 * shell sets it once its plugins are known; the toaster is made before it. */
let inPlace: (reason: string) => boolean = () => false;
export function answeredInPlace(is: (reason: string) => boolean): void {
  inPlace = is;
}

/** A refusal worth trying again as it was: wisp or the peer was busy or away, not a no. */
const RETRY = new Set(['timeout', 'unavailable', 'service_gone']);

/** The words for a refusal that came with none, by its code; the shell sets them once its strings load (the
 * toaster is made before it). Until then: the code itself, never a command id. */
let words: (code: string) => string | undefined = () => undefined;
let tryAgain = 'Try again';
export function errorWords(say: (code: string) => string | undefined, again: string): void {
  words = say;
  tryAgain = again;
}

/** ClientOptions.onError for createClient: the rollback toast, with Try again when trying again can help. */
export function commandErrorToast(toaster: Pick<Toaster, 'error'>): (err: CommandError, cmd: string, again?: () => void) => void {
  return (err, _cmd, again) => {
    if (err.reason !== undefined && inPlace(err.reason)) return;
    const text = err.message.trim() || words(err.code) || words('unknown') || err.code;
    toaster.error(text, again && (RETRY.has(err.code) || err.retry === true) ? { action: { label: tryAgain, onClick: again } } : undefined);
  };
}

export function wireNotices(ctx: ShellContext, owner: Scope): void {
  owner.add(
    ctx.client.peers.onNotice((_peer, n) => {
      if (n.kind !== 'toast') return;
      const d = (n.data ?? {}) as ToastNotice;
      if (typeof d.message !== 'string') return;
      const tone = d.tone === 'success' || d.tone === 'error' || d.tone === 'info' ? d.tone : 'info';
      const target = isTarget(d.target) ? d.target : undefined;
      ctx.toast.show(d.message, {
        tone,
        ...(typeof d.description === 'string' && { description: d.description }),
        ...(target && { action: { label: ctx.t('shell.toast.open'), onClick: () => ctx.navigate({ t: 'openFromToast', target }) } }),
      });
    }),
  );
}
