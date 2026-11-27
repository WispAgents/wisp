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
  return say(t, s, 'wisp.ship.steward.');
}

/** The same, for the Steward's own row whose title already says Steward ("Watching checks…", "Stopped: …"). */
export function stewardStatus(t: T, s: ShipSession): string | undefined {
  return say(t, s, 'wisp.ship.steward.s.');
}

function say(t: T, s: ShipSession, k: 'wisp.ship.steward.' | 'wisp.ship.steward.s.'): string | undefined {
  const st = s.steward;
  if (!st || s.pr?.state !== 'OPEN') return st?.step === 'stopped' && st.why ? t(`${k}stopped`, { why: st.why }) : undefined;
  switch (st.step) {
    case 'fixing':
      return t(`${k}fixing`, { what: causes(t, st), n: passes(st) });
    case 'pushing':
      return t(`${k}pushing`);
    case 'green':
      // green is for the head it judged, once its checks ran: a new push, or checks running again, is watching
      if (!greenNow(s, st)) return t(st.mode === 'review' ? `${k}watchingReview` : `${k}watching`);
      return st.reviewer ? t(`${k}greenBy`, { who: st.reviewer }) : t(`${k}green`);
    case 'stopped':
      return t(`${k}stopped`, { why: st.why || t('wisp.ship.steward.stoppedWhy') });
    default:
      return st.why ? t(`${k}waiting`, { why: st.why }) : t(st.mode === 'review' ? `${k}watchingReview` : `${k}watching`);
  }
}

/** The PR row's Steward switch (ADR-0132): on any open PR of the task's own branch, shipped with Ship it or attached
 * by the agent (`wisp_pr`). `on` while it watches, fixes or pushes; `next`: what flipping it sends, `off`, or else the
 * mode it had, else Settings → Ship's default (`ship.steward`), else `fix`. */
export function stewardSwitch(s: ShipSession, setting: unknown): { readonly on: boolean; readonly next: 'fix' | 'review' | 'off' } | undefined {
  if (s.pr?.state !== 'OPEN') return undefined;
  const st = s.steward;
  if (st && st.step !== 'stopped') return { on: true, next: 'off' };
  const mode = st?.mode ?? (typeof setting === 'string' ? setting : '');
  return { on: false, next: mode === 'review' ? 'review' : 'fix' };
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
