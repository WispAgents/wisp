// Inline delegate rows for a turn (`turn.ext['wisp.runtime'].delegations`), joined with each child session's
// live state so Chat mode (and Report mode) shows live progress and one-click navigation right on the turn. DOM-free.
import '../i18n/lazy.ts';
import type { T, TurnDoc } from './model.ts';

export interface DelegationSessionLike {
  readonly id: string;
  readonly state?: string;
  readonly agent?: string;
  readonly title?: string;
  readonly summary?: Readonly<Record<string, unknown>>;
}

export interface TurnDelegationRow {
  readonly id: string;
  readonly name: string;
  readonly agent: string;
  readonly line: string;
  readonly status: string;
  readonly tone: 'accent' | 'warn' | 'bad' | 'good' | 'neutral';
  readonly live: boolean;
  readonly running: boolean;
  readonly openable: boolean;
  readonly place?: string;
  readonly startedAt?: number;
  readonly endedAt?: number;
}

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
const str = (v: unknown): string => (typeof v === 'string' ? v : '');
const num = (v: unknown): number | undefined => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : undefined);

function stateOf(state: string, status: string, t: T): Pick<TurnDelegationRow, 'status' | 'tone' | 'live' | 'running'> {
  if (state === 'running') return { status: t('shell-task.delegations.working'), tone: 'accent', live: true, running: true };
  if (state === 'waiting') return { status: t('shell-task.delegations.asking'), tone: 'warn', live: false, running: true };
  if (status === 'error' || status === 'failed' || state === 'error') return { status: t('shell-task.delegations.failed'), tone: 'bad', live: false, running: false };
  if (status === 'stopped') return { status: t('shell-task.delegations.stopped'), tone: 'neutral', live: false, running: false };
  return { status: t('shell-task.delegations.done'), tone: 'good', live: false, running: false };
}

/** Build inline delegation rows for a turn, joining each recorded delegation with its live session row when present. */
export function delegationsOf(
  d: TurnDoc,
  sessions: readonly DelegationSessionLike[],
  harnessName: (id: string) => string,
  t: T,
): TurnDelegationRow[] {
  const raw = d.ext?.['wisp.runtime']?.delegations;
  if (!Array.isArray(raw) || raw.length === 0) return [];
  const byId = new Map<string, DelegationSessionLike>();
  for (const s of sessions) {
    byId.set(s.id, s);
    byId.set(bare(s.id), s);
  }
  const seen = new Set<string>();
  const out: TurnDelegationRow[] = [];
  for (const rec of raw) {
    if (!rec?.sessionId || seen.has(rec.sessionId)) continue;
    seen.add(rec.sessionId);
    const far = rec.sessionId.includes('|');
    const row = far ? undefined : byId.get(rec.sessionId);
    const sum = row?.summary ?? {};
    const agent = row?.agent || rec.agent || '';
    const harness = harnessName(agent);
    const model = str(sum['model']) || rec.model || '';
    const st = row
      ? stateOf(row.state ?? '', str(sum['status']), t)
      : stateOf(d.status === 'running' ? 'running' : 'done', '', t);
    const detail = st.live ? str(sum['live']) : str(sum['preview']);
    const line = [model ? `${harness} · ${model}` : harness, detail].filter(Boolean).join(' — ');
    const startedAt = num(sum['startedAt']) ?? num(rec.startedAt);
    const endedAt = num(sum['endedAt']);
    out.push({
      id: rec.sessionId,
      name: rec.name || row?.title || harness,
      agent,
      line,
      ...st,
      openable: !far,
      ...(rec.place && { place: rec.place }),
      ...(startedAt !== undefined && { startedAt }),
      ...(endedAt !== undefined && { endedAt }),
    });
  }
  return out;
}

/** Elapsed or final duration in ms for a delegation row, or undefined when no start time is recorded. */
export function delegationTime(r: Pick<TurnDelegationRow, 'running' | 'startedAt' | 'endedAt'>, now: number): number | undefined {
  if (r.startedAt === undefined) return undefined;
  if (r.running) return Math.max(0, now - r.startedAt);
  return r.endedAt !== undefined && r.endedAt >= r.startedAt ? r.endedAt - r.startedAt : undefined;
}
