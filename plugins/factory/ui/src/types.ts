// What this UI reads: a factory run's line on its task's list row (`factory`, wisp.factory's `runtime.summary`; the run's
// whole view is plugins/factory/api/bindings/factory/RunView.ts, kept on the task as `ext["wisp.factory"]`). The row
// carries each station's state, round, tasks and short output, and what the run waits on you for.

export const PLUGIN = 'wisp.factory';
export const KIND = { session: 'session' } as const;

/** factory/RunState.ts */
export type RunState = 'running' | 'waiting' | 'done' | 'failed' | 'stopped';
/** factory/StationState.ts */
export type StationState = 'waiting' | 'running' | 'asking' | 'done' | 'failed' | 'skipped';
export type StationId = 'spec' | 'build' | 'checks' | 'review' | 'ship';

export interface StationLine {
  readonly id: StationId;
  readonly state: StationState;
  readonly round: number;
  readonly tasks: readonly string[];
  readonly output: string;
  readonly why: string;
}

export interface RunLine {
  readonly name: string;
  readonly state: RunState;
  /** the station it's at ('' once it ended) */
  readonly station: string;
  readonly round: number;
  readonly max: number;
  readonly stations: readonly StationLine[];
  readonly why: string;
  readonly waiting?: { readonly station: string; readonly question: string; readonly options: readonly string[]; readonly askId: string };
  readonly pr?: string;
}

export interface SessionRow {
  readonly id: string;
  readonly factory?: RunLine;
  readonly summary?: Record<string, unknown>;
}
