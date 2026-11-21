// A factory run's band, worked out from its row (pure, so it's tested without a page): the header's words and tone,
// each station as a step (done, current, pending, skipped, failed) with what it gave, and the tasks to open.

import type { Key, Vars } from './i18n.ts';
import type { RunLine, StationId, StationLine } from './types.ts';
import './i18n/lazy.ts';

export type Tone = 'accent' | 'warn' | 'good' | 'bad' | 'neutral';
export type StepStatus = 'done' | 'current' | 'pending' | 'skipped' | 'failed';
export interface Said {
  readonly key: Key;
  readonly vars?: Vars;
}

export const STATION_KEY: Readonly<Record<StationId, Key>> = {
  spec: 'wisp.factory.station.spec',
  build: 'wisp.factory.station.build',
  checks: 'wisp.factory.station.checks',
  review: 'wisp.factory.station.review',
  ship: 'wisp.factory.station.ship',
};

const TONE = { running: 'accent', waiting: 'warn', done: 'good', failed: 'bad', stopped: 'neutral' } as const;

/** Whether the run still goes (Stop is offered). */
export const live = (r: RunLine): boolean => r.state === 'running' || r.state === 'waiting';

/** The header's pill: where it is (`Review · round 2 of 3`), that it waits on you, or how it ended. */
export function where(r: RunLine): { readonly said: Said; readonly tone: Tone } {
  const tone = TONE[r.state] ?? 'neutral';
  const at = r.stations.find((s) => s.id === r.station);
  if (r.state === 'waiting') return { said: { key: 'wisp.factory.waits' }, tone };
  if (r.state === 'running' && at) {
    const station = STATION_KEY[at.id];
    if (r.round > 1 && r.max > 0) return { said: { key: 'wisp.factory.at.round', vars: { station, round: r.round, max: r.max } }, tone };
    return { said: { key: 'wisp.factory.at', vars: { station } }, tone };
  }
  return { said: { key: `wisp.factory.state.${r.state === 'running' ? 'starting' : r.state}` as Key }, tone };
}

/** The band's state pill: only while the run goes. An ended run's state is its task's own status pill (Finished,
 * Failed), so the band doesn't say it a second time (audit G15: "Report" beside "Done"). */
export function pill(r: RunLine): ReturnType<typeof where> | undefined {
  return live(r) ? where(r) : undefined;
}

const STATUS: Readonly<Record<StationLine['state'], StepStatus>> = { done: 'done', failed: 'failed', skipped: 'skipped', running: 'current', asking: 'current', waiting: 'pending' };

/** One station as a step: its name (and round past the first), and what it gave, why it failed, or what it asks you. */
export function step(r: RunLine, s: StationLine): { readonly title: Said; readonly detail?: Said | string; readonly status: StepStatus } {
  const name = STATION_KEY[s.id];
  const title: Said = (s.round ?? 0) > 1 ? { key: 'wisp.factory.station.round', vars: { station: name, round: s.round } } : { key: name };
  const status = STATUS[s.state] ?? 'pending';
  // what it asks about, when it says (the spec waiting on your yes, audit G5), else the question
  if (s.state === 'asking' && r.waiting?.context) return { title, detail: { key: 'wisp.factory.asksAbout', vars: { about: r.waiting.context } }, status };
  if (s.state === 'asking' && r.waiting) return { title, detail: { key: 'wisp.factory.asks', vars: { question: r.waiting.question } }, status };
  // a station you stopped says Stopped, as the row and Home do (audit H tasks note: the core's token read "stopped")
  if (s.state === 'failed' && s.why === 'stopped') return { title, detail: { key: 'wisp.factory.state.stopped' }, status };
  if ((s.state === 'failed' || s.state === 'skipped') && s.why) return { title, detail: s.why, status };
  // running again: the last round's output isn't this round's
  if (s.state === 'running') return { title, detail: { key: 'wisp.factory.working' }, status };
  if (s.output) return { title, detail: s.output, status };
  return { title, status };
}

/** The station tasks to open, in order: `Build 2`'s when a station has several. */
export function openable(r: RunLine): readonly { readonly task: string; readonly said: Said }[] {
  return r.stations.flatMap((s) =>
    (s.tasks ?? []).map((task, i) => ({ task, said: (s.tasks ?? []).length > 1 ? { key: 'wisp.factory.open.nth' as Key, vars: { station: STATION_KEY[s.id], n: i + 1 } } : { key: 'wisp.factory.open' as Key, vars: { station: STATION_KEY[s.id] } } })),
  );
}

/** The run's line from a row, flattened (a synced row keeps plugin fields under `summary`). */
export function runOf(row: { readonly factory?: unknown; readonly summary?: Record<string, unknown> } | undefined): RunLine | undefined {
  const f = (row?.factory ?? row?.summary?.['factory']) as RunLine | undefined;
  return f && Array.isArray(f.stations) ? f : undefined;
}

/** Why the run ended, under the band: not when a station already says the same (audit G15: "ship failed: Nothing to
 *  ship…" on the Ship row and again under the band). */
export function endWhy(r: RunLine): string | undefined {
  if (!r.why || live(r)) return undefined;
  return r.stations.some((s) => s.why && (s.why === r.why || r.why.includes(s.why))) ? undefined : r.why;
}
