// What this UI reads: a task's workflow runs on its list row (`workflowRuns`, wisp.workflows' `runtime.summary`; the
// same views the task keeps as `ext["wisp.workflows"].runs`, plugins/workflows/host/src/run.rs `view`), and on a step's
// own task which run it belongs to (`workflowStep`). A synced row keeps plugin fields under `summary`.

export const PLUGIN = 'wisp.workflows';
export const KIND = { session: 'session' } as const;

export type RunState = 'running' | 'done' | 'failed' | 'stopped';
export type StepState = 'waiting' | 'running' | 'done' | 'failed' | 'skipped';

export interface StepView {
  /** `fix`, `fix[2]`, `audit[agent=codex]` */
  readonly id: string;
  /** the step it comes from */
  readonly step: string;
  readonly title?: string;
  readonly phase?: string;
  readonly agent?: string;
  readonly needs?: readonly string[];
  /** a foreach run's item (a text; anything else as short JSON) */
  readonly item?: string | null;
  readonly matrix?: Readonly<Record<string, unknown>>;
  /** a foreach step not made yet */
  readonly fanout?: boolean;
  readonly state: StepState;
  readonly task?: string | null;
  readonly attempts?: number;
  readonly why?: string;
  /** a running step whose turn is held before it starts (a limit, trust, the plan): why it waits */
  readonly held?: string;
  /** a line of what it gave */
  readonly said?: string;
  readonly startedAt?: number | null;
  readonly endedAt?: number | null;
  readonly cost?: number;
}

export interface RunView {
  readonly id: string;
  readonly workflow: string;
  readonly state: RunState;
  readonly cost?: number;
  readonly budget?: number | null;
  readonly description?: string;
  readonly steps: readonly StepView[];
  readonly phases?: readonly string[];
  readonly notes?: readonly string[];
  readonly done?: number;
  readonly total?: number;
  readonly startedAt?: number;
  readonly endedAt?: number | null;
  readonly held?: boolean;
  /** the workflow as it was given (its tasks, concurrency, budget) */
  readonly spec?: Readonly<Record<string, unknown>> | null;
}

export interface SessionRow {
  readonly id: string;
  readonly parentId?: string | null;
  readonly workflowRuns?: readonly RunView[];
  readonly workflowStep?: { readonly run: string; readonly node: string };
  readonly summary?: Record<string, unknown>;
}
