// The PR card's head when a task has several PRs: "2 PRs", "All merged", "1 conflict · 1 failing". Only the card
// (PrStrip, loaded lazily) reads it, so its words are the lazy half's (ADR-0011): its own module, so the pill's
// model/pr.ts stays in the first page without pulling en.lazy.json in with it.
import '../i18n/lazy.ts';
import type { T } from '../i18n.ts';
import type { ConflictFix, PrSummary, RepoPr } from '../types.ts';
import { fixing, prLine, prTone } from './pr.ts';

/** The PR card's title: "PR #840" for one PR, "2 PRs" for several. */
export const prStripTitle = (t: T, prs: readonly RepoPr[]): string => {
  const first = prs[0];
  if (!first) return '';
  return prs.length > 1 ? t('wisp.ship.strip.many', { count: prs.length }) : t('wisp.ship.tab.one', { n: first.pr.number });
};

/** The PR card's subtitle: the single PR's line, or an aggregated summary across all attached PRs. */
export function prStripSub(t: T, prs: readonly RepoPr[], ownPr?: PrSummary | null, fix?: ConflictFix | null): string {
  const first = prs[0];
  if (!first) return '';
  const fixFor = (x: RepoPr): ConflictFix | null | undefined => (ownPr && x.pr === ownPr ? fix : undefined);
  if (prs.length === 1) return prLine(t, first.pr, fixFor(first));
  if (prs.every((x) => x.pr.state === 'MERGED')) return t('wisp.ship.strip.allMerged');
  if (prs.every((x) => x.pr.state === 'CLOSED')) return t('wisp.ship.strip.allClosed');
  let conflicts = 0;
  let failing = 0;
  let running = 0;
  let open = 0;
  let merged = 0;
  let closed = 0;
  for (const x of prs) {
    const f = fixFor(x);
    const tone = prTone(x.pr, f);
    if (x.pr.state === 'MERGED') merged++;
    else if (x.pr.state === 'CLOSED') closed++;
    else if (x.pr.conflict && !fixing(f)) conflicts++;
    else if (tone === 'fail') failing++;
    else if (tone === 'pending') running++;
    else open++;
  }
  return [
    conflicts && t('wisp.ship.strip.conflicts', { count: conflicts }),
    failing && t('wisp.ship.strip.failing', { count: failing }),
    running && t('wisp.ship.strip.running', { count: running }),
    open && t('wisp.ship.strip.open', { count: open }),
    merged && t('wisp.ship.strip.merged', { count: merged }),
    closed && t('wisp.ship.strip.closed', { count: closed }),
  ]
    .filter(Boolean)
    .join(' · ');
}
