// Merge risk (wisp.risk, ADR-0140): another harness's judgment of each open PR, low · medium · high and why. One
// verdict per PR, of its head; the strip shows the worst of the task's open PRs (a task may have several, across
// repos), each PR's chip its own, and Merge's confirm warns about the risky ones. Pure; the verdicts come on the row
// (`risk`, with `riskOn` while Settings → Merge risk is on).
import type { IconName } from '@wisp/ui';
import type { T } from '../i18n.ts';
import type { PrSummary, RepoPr, RiskVerdict, ShipSession } from '../types.ts';
import '../i18n/lazy.ts';

/** Where one PR stands: being judged, judged (its level), the judge failed, or no verdict of its head yet. */
export type RiskKind = 'running' | 'none' | 'failed' | 'low' | 'medium' | 'high';

export const RISK_ICON = { running: 'clock', none: 'judge', failed: 'warn', low: 'check', medium: 'warn', high: 'warn' } as const satisfies Record<RiskKind, IconName>;

/** Worst first: what the strip shows for several PRs (a high one outweighs one still being judged). */
const RANK: Readonly<Record<RiskKind, number>> = { high: 6, medium: 5, running: 4, failed: 3, none: 2, low: 1 };

/** Not judged, or the judge failed: the strip's button starts a judgment. Otherwise it opens the PR pane. */
export const riskActs = (k: RiskKind): boolean => k === 'none' || k === 'failed';

/** The verdict of this PR's head, when there is one. */
export function verdictOf(s: ShipSession | undefined, pr: PrSummary): RiskVerdict | undefined {
  return (s?.risk ?? []).find((v) => v.url === pr.url && v.current);
}

export function riskKind(v: RiskVerdict | undefined): RiskKind {
  if (!v) return 'none';
  if (v.status === 'running') return 'running';
  if (v.status === 'failed' || !v.level) return 'failed';
  return v.level;
}

/** "#412 High (78/100): drops a column" — one PR's verdict in a line, for titles and the confirm. */
export function riskLine(t: T, x: RepoPr, v: RiskVerdict | undefined, several: boolean): string {
  const k = riskKind(v);
  const pr = several ? `${x.repo ? `${x.repo} ` : ''}#${x.pr.number} ` : '';
  const score = v?.score !== undefined && k !== 'running' && k !== 'failed' ? ` (${v.score}/100)` : '';
  const why = k === 'failed' ? v?.why : k === 'none' || k === 'running' ? '' : v?.summary;
  return `${pr}${t(`wisp.ship.risk.${k}`)}${score}${why ? `: ${why}` : ''}`;
}

export interface RiskStrip {
  readonly kind: RiskKind;
  readonly label: string;
  /** every PR's line, one per line */
  readonly title: string;
  /** PRs with no verdict of their head, or a failed one: what Judge starts */
  readonly toJudge: number;
}

/** The strip's merge-risk button over the task's open PRs, or undefined while the setting is off or none is open. */
export function riskStrip(t: T, s: ShipSession | undefined, open: readonly RepoPr[]): RiskStrip | undefined {
  if (!s?.riskOn || open.length === 0) return undefined;
  const several = open.length > 1;
  const each = open.map((x) => ({ x, v: verdictOf(s, x.pr) }));
  const kinds = each.map((e) => riskKind(e.v));
  const kind = kinds.reduce<RiskKind>((w, k) => (RANK[k] > RANK[w] ? k : w), 'low');
  const lines = each.map((e) => riskLine(t, e.x, e.v, several));
  const hint = riskActs(kind) ? t('wisp.ship.risk.clickJudge') : t('wisp.ship.risk.clickOpen');
  const bad = kinds.filter((k) => k === kind).length;
  // several PRs: "High risk · 2" says how many share the worst
  const label = several && bad > 1 && (kind === 'high' || kind === 'medium') ? `${t(`wisp.ship.risk.${kind}`)} · ${bad}` : t(`wisp.ship.risk.${kind}`);
  return { kind, label, title: [...lines, hint].join('\n'), toJudge: kinds.filter(riskActs).length };
}

/** One PR's chip suffix ("high risk"), when the setting is on and its head is judged; nothing while unjudged. */
export function chipRisk(t: T, s: ShipSession | undefined, pr: PrSummary): string | undefined {
  if (!s?.riskOn) return undefined;
  const k = riskKind(verdictOf(s, pr));
  return k === 'low' || k === 'medium' || k === 'high' ? t(`wisp.ship.risk.short.${k}`) : undefined;
}

/** Merge's confirm: a line per open PR judged high (red, Merge anyway) and per one judged medium (a line only). */
export function riskWarnings(t: T, s: ShipSession | undefined, open: readonly RepoPr[]): { readonly high: string[]; readonly medium: string[] } {
  const out = { high: [] as string[], medium: [] as string[] };
  if (!s?.riskOn) return out;
  const several = open.length > 1;
  for (const x of open) {
    const v = verdictOf(s, x.pr);
    const k = riskKind(v);
    if (k === 'high' || k === 'medium') out[k].push(t('wisp.ship.risk.warn', { line: riskLine(t, x, v, several) }));
  }
  return out;
}
