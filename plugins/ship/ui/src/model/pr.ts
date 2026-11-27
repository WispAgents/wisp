// A task's PRs and their words (v1 ship.js prTone / prLine / shippedPrs / prsOf / shippable /
// shipLabel): which PRs a row has, the tone each is in, its one line, and whether Ship it applies.
// Pure: the pill, the strip, the dialog and the pane all read these.
import type { T } from '../i18n.ts';
import type { ConflictFix, PrSummary, RepoPr, ShipSession } from '../types.ts';
import { prsOf, shippedPrs } from './prs.ts';

export type PrTone = 'fail' | 'pending' | 'pass' | 'merged' | 'closed';

/** Worst first: the row takes the worst of its PRs. */
const ORDER: readonly PrTone[] = ['fail', 'pending', 'pass', 'merged', 'closed'];

/** wisp is resolving the PR's merge conflicts itself. */
export const fixing = (fix: ConflictFix | undefined | null): boolean => fix?.step === 'fixing' || fix?.step === 'pushing';

export function prTone(pr: PrSummary, fix?: ConflictFix | null): PrTone {
  if (pr.state === 'MERGED') return 'merged';
  if (pr.state === 'CLOSED') return 'closed';
  if (pr.conflict) return fixing(fix) ? 'pending' : 'fail';
  if (pr.checks?.fail) return 'fail';
  if (pr.checks?.pending || pr.waitingChecks) return 'pending';
  return pr.review === 'CHANGES_REQUESTED' ? 'fail' : 'pass';
}

export const worstTone = (tones: readonly PrTone[]): PrTone => [...tones].sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b))[0] ?? 'pass';

const REVIEW = { APPROVED: 'wisp.ship.pr.approved', CHANGES_REQUESTED: 'wisp.ship.pr.changes', REVIEW_REQUIRED: 'wisp.ship.pr.reviewRequired' } as const;

function conflictLine(t: T, fix: ConflictFix | null | undefined): string {
  const what =
    fix?.step === 'pushing' ? t('wisp.ship.pr.pushingFix')
    : fixing(fix) ? t('wisp.ship.pr.fixing')
    : fix?.step === 'failed' ? t('wisp.ship.pr.notFixed', { why: fix.why ?? '' })
    : t('wisp.ship.pr.conflicts');
  return `${t('wisp.ship.pr.open')} · ${what}`;
}

/** "Open · 2 checks failing · Changes requested" (v1 prLine). */
export function prLine(t: T, pr: PrSummary, fix?: ConflictFix | null): string {
  if (pr.state === 'OPEN' && (pr.conflict || fixing(fix))) return conflictLine(t, fix);
  const c = pr.checks;
  const checks = c?.fail ? t('wisp.ship.pr.failing', { count: c.fail }) : c?.pending ? t('wisp.ship.pr.running', { count: c.pending }) : c?.pass ? t('wisp.ship.pr.passed', { count: c.pass }) : pr.waitingChecks ? t('wisp.ship.pr.waitingChecks') : t('wisp.ship.pr.noChecks');
  const state = pr.state === 'MERGED' ? t('wisp.ship.pr.merged') : pr.state === 'CLOSED' ? t('wisp.ship.pr.closed') : pr.draft ? t('wisp.ship.pr.draft') : t('wisp.ship.pr.open');
  const review = pr.state === 'OPEN' && pr.review ? t(REVIEW[pr.review]) : '';
  return [state, pr.state === 'OPEN' ? checks : '', review].filter(Boolean).join(' · ');
}

/** The pill's tooltip: "PR #412: Open · 3 checks passed". */
export const pillTitle = (t: T, pr: PrSummary, fix?: ConflictFix | null): string => t('wisp.ship.pill.title', { n: pr.number, line: prLine(t, pr, fix) });

/** One PR chip's label: "wisp #840", or "#840" when no repo name is known (never a leading space). */
export const prChipLabel = (x: RepoPr): string => (x.repo ? `${x.repo} #${x.pr.number}` : `#${x.pr.number}`);

/** One PR chip's tooltip: "wisp: PR #840: Merged", or "PR #840: Merged" when no repo name is known. */
export const prChipTitle = (t: T, x: RepoPr, fix?: ConflictFix | null): string => {
  const line = pillTitle(t, x.pr, fix);
  return x.repo ? `${x.repo}: ${line}` : line;
};

// the row's PRs, what the pill's `if` reads as the list draws: their own module, so this one loads with the pill
export { prOf, prsOf, shippedPrs } from './prs.ts';
export const openPrs = (s: ShipSession | undefined): RepoPr[] => prsOf(s).filter((x) => x.pr.state === 'OPEN');

/** Its branches: the task's own, then its other repos'. */
export const branchesOf = (s: ShipSession | undefined): string[] => [s?.branch, ...(s?.also ?? []).map((a) => a.branch)].filter((b): b is string => !!b);

/**
 * Ship it applies to a finished worktree task with something to ship: uncommitted changes, or
 * commits its open PR (or a new one) doesn't have. Not checked yet: it might. After its PR merged,
 * only work since then. Several repos: the server counts the ones with something new.
 */
export function shippable(s: ShipSession | undefined, sending = false): boolean {
  if (!s || !branchesOf(s).length || s.agent === 'wisp' || s.state === 'running' || s.state === 'waiting' || s.shipping || sending) return false;
  const ts = s.toShip;
  if (ts?.repos !== undefined) return ts.repos > 0;
  if (s.pr?.state === 'MERGED') return !!(ts?.dirty || ts?.unpushed);
  return !ts || !!(ts.dirty || (s.pr?.state === 'OPEN' ? ts.unpushed : ts.ahead));
}

/** The action's one name, in the bar, the menus and ⌘K. */
export function shipLabel(t: T, s: ShipSession): string {
  if (branchesOf(s).length > 1) return shippedPrs(s).some((x) => x.pr.state === 'OPEN') ? t('wisp.ship.action.pushMany') : t('wisp.ship.action.ship');
  return s.pr?.state === 'OPEN' ? t('wisp.ship.action.push', { n: s.pr.number }) : t('wisp.ship.action.ship');
}
