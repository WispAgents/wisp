// A row's PRs (v1 ship.js shippedPrs / prsOf): what the PR pill's `if` reads while the list draws, so this is all of
// the model the first page loads (budget, ADR-0011); the words and the rest (pr.ts) load with the pill.
import type { PrSummary, RepoPr, ShipSession } from '../types.ts';

/** The PRs Ship it opened: the task's own first, then its other repos'. */
export function shippedPrs(s: ShipSession | undefined): RepoPr[] {
  if (!s) return [];
  const own: RepoPr[] = s.pr ? [{ repo: s.project, pr: s.pr }] : [];
  const also = (s.also ?? []).flatMap((a): RepoPr[] => (a.pr ? [{ repo: a.project, pr: a.pr }] : []));
  return [...own, ...also];
}

const prKey = (x: RepoPr): string => x.pr.url || `${x.repo ?? ''}#${x.pr.number}`;

/** Every PR the row shows: shipped ones, then the ones its agent attached (wisp_pr), deduplicated. */
export function prsOf(s: ShipSession | undefined): RepoPr[] {
  const seen = new Set<string>();
  const out: RepoPr[] = [];
  for (const x of [...shippedPrs(s), ...(s?.prs ?? []).map((p): RepoPr => ({ repo: p.repo, pr: p.pr }))]) {
    const k = prKey(x);
    if (seen.has(k)) continue;
    seen.add(k);
    out.push(x);
  }
  return out;
}

/** The PR the row's pill shows. */
export const prOf = (s: ShipSession | undefined): PrSummary | undefined => prsOf(s)[0]?.pr;
