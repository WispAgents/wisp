// The PR pane's merge risk (wisp.risk, ADR-0140), DOM-free: the picked PR's verdict from the task's row (`risk`, one
// per PR, with `riskOn` while Settings → Merge risk is on), and what the section says about it.

export type RiskLevel = 'low' | 'medium' | 'high';

/** One PR's merge risk as the row has it (wisp-risk-api RiskView). */
export interface RiskVerdict {
  readonly url: string;
  readonly head: string;
  readonly status: 'running' | 'done' | 'failed';
  readonly level?: RiskLevel;
  readonly score?: number;
  readonly summary?: string;
  readonly reasons?: readonly string[];
  readonly by?: { readonly agent: string; readonly model: string };
  readonly why?: string;
  readonly partial?: boolean;
  readonly current: boolean;
}

type RiskFields = { readonly riskOn?: boolean; readonly risk?: readonly RiskVerdict[] };

/** The row's merge-risk fields, under `summary` on a server row, on itself on a flattened one. */
function fieldsOf(row: unknown): RiskFields {
  const top = (row ?? {}) as RiskFields & { readonly summary?: RiskFields };
  return top.riskOn !== undefined || top.risk !== undefined ? top : (top.summary ?? {});
}

export type RiskState = 'off' | 'none' | 'running' | 'failed' | 'stale' | RiskLevel;

export interface RiskSection {
  readonly state: RiskState;
  readonly verdict?: RiskVerdict;
}

/** What the pane shows for the PR at `url` (open PRs only): `off` hides the section. `stale`: judged, then pushed to. */
export function riskSection(row: unknown, url: string, open: boolean): RiskSection {
  const f = fieldsOf(row);
  if (!f.riskOn || !open) return { state: 'off' };
  const v = (f.risk ?? []).find((x) => x.url === url);
  if (!v) return { state: 'none' };
  if (v.status === 'running') return { state: v.current ? 'running' : 'stale', verdict: v };
  if (!v.current) return { state: 'stale', verdict: v };
  if (v.status === 'failed' || !v.level) return { state: 'failed', verdict: v };
  return { state: v.level, verdict: v };
}

/** The section's tone, in the pane's banner tones. */
export const RISK_TONE: Readonly<Record<Exclude<RiskState, 'off'>, 'good' | 'warn' | 'bad' | 'neutral'>> = {
  low: 'good',
  medium: 'warn',
  high: 'bad',
  running: 'neutral',
  none: 'neutral',
  failed: 'warn',
  stale: 'neutral',
};
