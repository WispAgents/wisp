// The board's view kind, apart from the views' code so the plugin's index needs only this (ADR-0011).
export const VIEW = {
  /** main: a task's workflow runs as the full board, in the task's place; params { task, run? } */
  board: 'workflows.board',
} as const;

export interface BoardParams {
  /** the task's entity key */
  readonly task: string;
  /** the run to show first (default: the one going, else the newest) */
  readonly run?: string;
}

export function parseBoard(p: unknown): BoardParams {
  const o = p as { task?: unknown; run?: unknown } | null;
  if (!o || typeof o.task !== 'string' || !o.task.includes(':')) throw new Error('workflow board: task key');
  return typeof o.run === 'string' ? { task: o.task, run: o.run } : { task: o.task };
}
