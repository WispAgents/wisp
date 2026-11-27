// The Steward's words on the PR row (ADR-0132), DOM-free: what it's doing, and the one action it offers.
import type { T } from '../i18n.ts';
import type { ShipSession, Steward } from '../types.ts';
import '../i18n/lazy.ts';

/** All passes so far, every cause. */
const passes = (s: Steward): number => (s.passes?.checks ?? 0) + (s.passes?.threads ?? 0) + (s.passes?.review ?? 0);

/** What it's fixing in this pass, in words ("failing checks, review comments"). */
function causes(t: T, s: Steward): string {
  const words = (s.causes ?? []).map((c) => t(c === 'checks' ? 'wisp.ship.steward.cause.checks' : c === 'threads' ? 'wisp.ship.steward.cause.threads' : 'wisp.ship.steward.cause.review'));
  return words.length ? words.join(', ') : t('wisp.ship.steward.cause.pr');
}

/** The Steward's green still holds: the PR's head is the one it judged, and no check is running or still to show. */
function greenNow(s: ShipSession, st: Steward): boolean {
  const pr = s.pr;
  if (!pr) return false;
  if (st.head && pr.head && st.head !== pr.head) return false;
  return !pr.checks?.pending && !pr.checks?.fail && !pr.waitingChecks;
}

/** The line under the PR row while the Steward is on (or just stopped); undefined without one. */
export function stewardLine(t: T, s: ShipSession): string | undefined {
  const st = s.steward;
  if (!st || s.pr?.state !== 'OPEN') return st?.step === 'stopped' && st.why ? t('wisp.ship.steward.stopped', { why: st.why }) : undefined;
  switch (st.step) {
    case 'fixing':
      return t('wisp.ship.steward.fixing', { what: causes(t, st), n: passes(st) });
    case 'pushing':
      return t('wisp.ship.steward.pushing');
    case 'green':
      // green is for the head it judged, once its checks ran: a new push, or checks running again, is watching
      if (!greenNow(s, st)) return t(st.mode === 'review' ? 'wisp.ship.steward.watchingReview' : 'wisp.ship.steward.watching');
      return st.reviewer ? t('wisp.ship.steward.greenBy', { who: st.reviewer }) : t('wisp.ship.steward.green');
    case 'stopped':
      return t('wisp.ship.steward.stopped', { why: st.why || t('wisp.ship.steward.stoppedWhy') });
    default:
      return st.why ? t('wisp.ship.steward.waiting', { why: st.why }) : t(st.mode === 'review' ? 'wisp.ship.steward.watchingReview' : 'wisp.ship.steward.watching');
  }
}

/** The row's one Steward control: Stop while it's on, Start again after a stop, Keep green on an open PR without one. */
export function stewardAction(s: ShipSession): 'stop' | 'again' | 'start' | undefined {
  if (s.pr?.state !== 'OPEN') return undefined;
  const st = s.steward;
  if (!st) return 'start';
  return st.step === 'stopped' ? 'again' : 'stop';
}

/** The dialog's two boxes as `ship.ship`'s `steward`: unticked sends `off` only when one is on now. */
export function stewardArg(on: boolean, review: boolean, now: Steward | undefined): 'fix' | 'review' | 'off' | undefined {
  if (on) return review ? 'review' : 'fix';
  return now ? 'off' : undefined;
}

/** The boxes' first state: the task's own Steward, else Settings → Ship's default (`ship.steward`). */
export function stewardStart(now: Steward | undefined, setting: unknown): { on: boolean; review: boolean } {
  if (now && now.step !== 'stopped') return { on: true, review: now.mode === 'review' };
  const v = typeof setting === 'string' ? setting : '';
  return { on: v === 'fix' || v === 'review', review: v === 'review' };
}
