// The board's model (ADR-0119), pure: a run view as the kit's RunBoard draws it. Its phases are lanes in the order they
// first appear; a step that ran once is a card, one that fanned out (a foreach, a matrix of more than three) is one card
// with a dot per run. Each card says how long it took or has been going, its cost, and what it gave or why not.

import type { RunEntry, RunFan, RunLane, RunStep, RunStepState } from '@wisp/ui';
import { t } from './i18n.ts';
import type { RunState, RunView, SessionRow, StepView } from './types.ts';

export type Tone = 'accent' | 'warn' | 'good' | 'bad' | 'neutral';

/** A matrix this size or smaller shows a card per run (claude and codex side by side); larger ones are a fan. */
export const CARDS_MAX = 3;

/** The runs a row carries, oldest first (a synced row keeps plugin fields under `summary`). */
export function runsOf(row: SessionRow | undefined): readonly RunView[] {
  const r = (row?.workflowRuns ?? row?.summary?.['workflowRuns']) as unknown;
  return Array.isArray(r) ? (r as RunView[]).filter((x) => x && typeof x.id === 'string' && Array.isArray(x.steps)) : [];
}

/** The task a step's task is under (a synced row says `parent_id`, its summary `parentId`). */
export function parentOf(row: SessionRow | undefined): string | undefined {
  const p = row?.parentId ?? row?.parent_id ?? row?.summary?.['parentId'];
  return typeof p === 'string' && p ? p : undefined;
}

/** The run a step's task is part of (`workflowStep`), or undefined. */
export function stepOf(row: SessionRow | undefined): { readonly run: string; readonly node: string } | undefined {
  const s = (row?.workflowStep ?? row?.summary?.['workflowStep']) as { run?: unknown; node?: unknown } | undefined;
  return s && typeof s.run === 'string' && typeof s.node === 'string' ? { run: s.run, node: s.node } : undefined;
}

export const live = (r: RunView): boolean => r.state === 'running';

/** `12s`, `3m 04s`, `1h 02m` */
export function duration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, '0')}s`;
  return `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, '0')}m`;
}

/** `$0.42` (cents under a dollar keep two places; nothing when it cost nothing yet) */
export function money(usd: number | undefined | null): string {
  return usd && usd > 0 ? `$${usd < 10 ? usd.toFixed(2) : usd.toFixed(1)}` : '';
}

/** How long a step has run (ended: how long it took); '' before it started. */
export function took(s: Pick<StepView, 'startedAt' | 'endedAt' | 'state'>, now: number): string {
  if (!s.startedAt) return '';
  const end = s.endedAt ?? (s.state === 'running' ? now : undefined);
  return end ? duration(end - s.startedAt) : '';
}

const ended = (s: StepView): boolean => s.state === 'done' || s.state === 'failed' || s.state === 'skipped';

/** What a group of steps comes to: running while one runs, failed once one failed and none runs, done when all ended. */
export function stateOf(steps: readonly Pick<StepView, 'state'>[]): RunStepState {
  if (steps.some((s) => s.state === 'running')) return 'running';
  if (steps.length > 0 && steps.every((s) => s.state === 'skipped')) return 'skipped';
  if (steps.some((s) => s.state === 'failed') && steps.every((s) => s.state !== 'waiting')) return 'failed';
  if (steps.length > 0 && steps.every((s) => s.state === 'done' || s.state === 'skipped')) return 'done';
  return 'waiting';
}

/** A step's name on its card: its title, else its step; a run of a foreach its item, of a matrix its values. */
export function nameOf(s: StepView): string {
  if (s.item !== undefined && s.item !== null && s.item !== '') return String(s.item);
  const m = Object.entries(s.matrix ?? {}).filter(([k]) => k !== 'item' && k !== 'index');
  if (m.length > 0) return m.map(([, v]) => (typeof v === 'string' ? v : JSON.stringify(v))).join(' · ');
  return s.title || s.step;
}

/** Its line under the title: working, what it gave, why it failed or was skipped, or what it waits on. */
export function detailOf(s: StepView): string {
  if (s.fanout && s.state === 'waiting') return t('wisp.workflows.fansOut', { needs: (s.needs ?? []).join(', ') || '…' });
  if (s.state === 'running' && s.held) return t('wisp.workflows.held', { why: s.held });
  if (s.state === 'running') return (s.attempts ?? 1) > 1 ? t('wisp.workflows.tryN', { n: s.attempts ?? 2 }) : t('wisp.workflows.working');
  if (s.state === 'done') return s.said ?? '';
  if (s.state === 'failed' || s.state === 'skipped') return s.why ?? '';
  return (s.needs ?? []).length > 0 ? t('wisp.workflows.after', { needs: (s.needs ?? []).join(', ') }) : t('wisp.workflows.queued');
}

function meta(s: StepView, now: number): string {
  return [took(s, now), money(s.cost)].filter(Boolean).join(' · ');
}

/** One step as a card; `open` opens its task (none until it has one). */
/** What the board's cards do: open a step's task, and change the run (Skip, Retry; none: the cards only open). */
export interface Hands {
  readonly open: (task: string) => void;
  readonly act?: (kind: 'skip' | 'retry', ids: readonly string[]) => void;
}

/** What you can do to a step in this state: skip one that waits or works, retry one that failed or was skipped. */
export function stepActions(state: StepView['state']): { value: 'skip' | 'retry'; label: string; icon: 'skip' | 'rerun' }[] {
  if (state === 'waiting' || state === 'running') return [{ value: 'skip', label: t('wisp.workflows.skip'), icon: 'skip' }];
  if (state === 'failed' || state === 'skipped') return [{ value: 'retry', label: t('wisp.workflows.retry'), icon: 'rerun' }];
  return [];
}

export function card(s: StepView, now: number, h: Hands, withStep = false): RunStep {
  const title = withStep ? `${s.title || s.step} · ${nameOf(s)}` : nameOf(s);
  const task = s.task;
  const detail = detailOf(s);
  const m = meta(s, now);
  return {
    id: s.id,
    title,
    // held before it starts: it waits, it isn't working
    state: s.state === 'running' && s.held ? 'waiting' : s.state,
    ...(s.agent ? { agent: s.agent } : {}),
    ...(detail ? { detail } : {}),
    ...(m ? { meta: m } : {}),
    ...(task ? { onOpen: () => h.open(task), openLabel: t('wisp.workflows.open', { step: title }) } : {}),
    ...(h.act && stepActions(s.state).length > 0 ? { actions: stepActions(s.state), onAction: (v: string) => h.act?.(v === 'skip' ? 'skip' : 'retry', [s.id]) } : {}),
  };
}

/** A fan-out's actions: retry the runs that failed or were skipped, skip the ones that wait or work. */
function fanActions(runs: readonly StepView[], h: Hands): Pick<RunFan, 'actions' | 'onAction'> {
  if (!h.act) return {};
  const again = runs.filter((r) => r.state === 'failed' || r.state === 'skipped').map((r) => r.id);
  const rest = runs.filter((r) => r.state === 'waiting' || r.state === 'running').map((r) => r.id);
  const actions = [
    ...(again.length ? [{ value: 'retry', label: t('wisp.workflows.retryN', { count: again.length }), icon: 'rerun' as const }] : []),
    ...(rest.length ? [{ value: 'skip', label: t('wisp.workflows.skipN', { count: rest.length }), icon: 'skip' as const }] : []),
  ];
  return actions.length ? { actions, onAction: (v: string) => h.act?.(v === 'skip' ? 'skip' : 'retry', v === 'skip' ? rest : again) } : {};
}

/** A lane's entries: steps in order, the runs of one step together. */
function entries(steps: readonly StepView[], now: number, h: Hands): RunEntry[] {
  const order: string[] = [];
  const by = new Map<string, StepView[]>();
  for (const s of steps) {
    if (!by.has(s.step)) {
      order.push(s.step);
      by.set(s.step, []);
    }
    by.get(s.step)?.push(s);
  }
  return order.flatMap((name): RunEntry[] => {
    const runs = by.get(name) ?? [];
    const first = runs[0];
    if (!first) return [];
    const foreach = runs.some((r) => r.item !== undefined && r.item !== null);
    if (runs.length === 1 && !foreach) return [{ step: card(first, now, h) }];
    if (!foreach && runs.length <= CARDS_MAX) return runs.map((r) => ({ step: card(r, now, h, true) }));
    const doneN = runs.filter(ended).length;
    const failed = runs.filter((r) => r.state === 'failed').length;
    const running = runs.filter((r) => r.state === 'running').length;
    const detail = failed > 0 ? t('wisp.workflows.fan.failed', { count: failed }) : running > 0 ? t('wisp.workflows.fan.running', { count: running }) : '';
    return [
      {
        fan: {
          id: name,
          title: first.title || name,
          state: stateOf(runs),
          ...(first.agent ? { agent: first.agent } : {}),
          count: t('wisp.workflows.fan.count', { done: doneN, total: runs.length }),
          ...(detail ? { detail } : {}),
          items: runs.map((r) => card(r, now, { open: h.open })),
          ...fanActions(runs, h),
        },
      },
    ];
  });
}

/** The board's lanes: one per phase, in the order phases first appear (one lane, no header, when none has a phase). */
export function lanes(r: RunView, now: number, h: Hands): RunLane[] {
  const phases: string[] = [];
  for (const s of r.steps) if (!phases.includes(s.phase ?? '')) phases.push(s.phase ?? '');
  return phases.map((p) => {
    const mine = r.steps.filter((s) => (s.phase ?? '') === p);
    const doneN = mine.filter(ended).length;
    return { id: p || '-', title: phases.length === 1 && !p ? '' : p || t('wisp.workflows.otherPhase'), count: `${doneN}/${mine.length}`, state: stateOf(mine), entries: entries(mine, now, h) };
  });
}

const TONE: Readonly<Record<RunState, Tone>> = { running: 'accent', done: 'good', failed: 'bad', stopped: 'neutral' };

/** The phase a running run is in (the first lane still going), '' without phases. */
export function phaseNow(r: RunView): string {
  const going = r.steps.find((s) => s.state === 'running') ?? r.steps.find((s) => s.state === 'waiting');
  return going?.phase ?? '';
}

/** The header's pill: where it is while it goes (`Build · 4/9`), else how it ended. */
export function pill(r: RunView): { readonly text: string; readonly tone: Tone; readonly live: boolean } {
  const n = { done: r.done ?? r.steps.filter(ended).length, total: r.total ?? r.steps.length };
  if (live(r)) {
    return { text: t('wisp.workflows.progress', n), tone: TONE.running, live: true };
  }
  return { text: t(`wisp.workflows.state.${r.state}`), tone: TONE[r.state] ?? 'neutral', live: false };
}

/** The header's quiet line: how long, what it cost (of its budget), how many steps. */
export function line(r: RunView, now: number): string {
  const end = r.endedAt ?? (live(r) ? now : undefined);
  const parts = [
    r.startedAt && end ? duration(end - r.startedAt) : '',
    r.budget ? t('wisp.workflows.costOf', { cost: money(r.cost) || '$0.00', budget: money(r.budget) }) : money(r.cost),
    live(r) ? '' : t('wisp.workflows.steps', { count: r.total ?? r.steps.length }),
  ];
  return parts.filter(Boolean).join(' · ');
}

/** The share of steps that ended, for the bar (0..1). */
export function share(r: RunView): number {
  const total = r.total ?? r.steps.length;
  return total > 0 ? (r.done ?? r.steps.filter(ended).length) / total : 0;
}

/** The run's workflow as a formula to keep and run again (`wisp apply`, then `/workflow <name>`): JSON is YAML. */
export function asFormula(r: RunView): string {
  const spec = r.spec && typeof r.spec === 'object' ? r.spec : { tasks: {} };
  return JSON.stringify({ wisp: 1, workflows: [{ name: r.workflow, ...spec }] }, null, 2);
}

/** At most this many dots on the card; past it, the last says how many more. */
export const DOTS_MAX = 48;

/** The card's strip: a dot per step run, in order (a foreach's each run, a matrix's each agent). */
export function dots(r: RunView): { id: string; title: string; state: RunStepState; detail?: string }[] {
  const all = r.steps.map((s) => {
    const title = s.item !== undefined && s.item !== null && s.item !== '' ? `${s.title || s.step} · ${nameOf(s)}` : nameOf(s);
    const detail = detailOf(s);
    return { id: s.id, title, state: s.state, ...(detail ? { detail } : {}) };
  });
  if (all.length <= DOTS_MAX) return all;
  const rest = all.slice(DOTS_MAX - 1);
  return [...all.slice(0, DOTS_MAX - 1), { id: '+', title: t('wisp.workflows.moreSteps', { count: rest.length }), state: stateOf(rest) }];
}

/** The card's quiet line: the phase it's in (while it goes), how long, what it cost, and the task's other runs. */
export function cardLine(r: RunView, now: number, more: number): string {
  const p = live(r) ? phaseNow(r) : '';
  return [p, line(r, now), more > 0 ? t('wisp.workflows.moreRuns', { count: more }) : ''].filter(Boolean).join(' · ');
}
