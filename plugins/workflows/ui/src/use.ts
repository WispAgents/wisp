// What the card and the board share: the task's row and runs, a clock for live durations, opening a step's task,
// opening the board, and Stop.
import { createEffect, createMemo, createSignal, onCleanup } from 'solid-js';
import { defineCommand, type EntityKey, type PluginClient } from '@wisp/client';
import { useSignal } from '@wisp/ui';
import { live, runsOf } from './model.ts';
import { KIND, type RunView, type SessionRow } from './types.ts';
import { VIEW } from './views.ts';

/** `workflows.stop`: the run's working steps stop; the view says so once the row does (the button shows it's pending). */
export const stopRun = defineCommand<{ session: EntityKey; run: string }, unknown>({
  id: 'workflows.stop',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.run }),
  optimistic: 'none',
  pending: 'the workflow run',
});

export const keyOf = (client: PluginClient, id: string): EntityKey => (id.includes(':') ? id : `${client.home}:${id}`) as EntityKey;

/** A clock for the live durations: ticks each second while a run goes, on the client's scope (stops when none does). */
export function useNow(c: PluginClient, going: () => boolean): () => number {
  const [now, setNow] = createSignal(c.scope.timers.now());
  createEffect(() => {
    if (!going()) return;
    setNow(c.scope.timers.now());
    onCleanup(c.scope.interval(1000, () => setNow(c.scope.timers.now())));
  });
  return now;
}

/** The task's row and its runs (oldest first), live. */
export function useRuns(c: PluginClient, k: EntityKey): { row: () => SessionRow | undefined; runs: () => readonly RunView[] } {
  const row = useSignal(c.entities.row<SessionRow>(KIND.session, k)) as () => SessionRow | undefined;
  const runs = createMemo(() => runsOf(row()));
  return { row, runs };
}

/** The run a task's card shows: one going (the newest of those), else the newest. */
export const current = (runs: readonly RunView[]): RunView | undefined => [...runs].reverse().find(live) ?? runs[runs.length - 1];

/** Opens a task by its bare id, on the peer of `k`. */
export const openTask = (c: PluginClient, k: EntityKey, task: string): void => c.navigate({ t: 'openTask', id: `${k.slice(0, k.indexOf(':'))}:${task}` as never });

/** Opens the task's workflow board in the main area (Back, or the board's own back, returns to the task). */
export const openBoard = (c: PluginClient, k: EntityKey, run?: string): void =>
  c.navigate({ t: 'openView', id: VIEW.board as never, params: run ? { task: k, run } : { task: k } });

/** `workflows.edit` from the board's cards: skip or retry steps (the board shows it once the row does). */
export const editRun = defineCommand<{ session: EntityKey; run: string; skip?: readonly string[]; retry?: readonly string[] }, unknown>({
  id: 'workflows.edit',
  owner: (a) => a.session,
  wire: (a) => ({ id: a.run, ...(a.skip?.length ? { skip: a.skip } : {}), ...(a.retry?.length ? { retry: a.retry } : {}) }),
  optimistic: 'none',
  pending: 'the workflow run',
});

/** `workflows.set`: the run's workflow kept as yours under its name (then `/workflow <name>`, `wisp export workflows`). */
export const saveWorkflow = defineCommand<{ session: EntityKey; name: string; spec: Readonly<Record<string, unknown>> }, unknown>({
  id: 'workflows.set',
  owner: (a) => a.session,
  wire: (a) => ({ ...a.spec, name: a.name }),
  optimistic: 'none',
  pending: 'the workflow',
});

/** A run's name as a workflow's (letters, digits, - _ .). */
export const savedName = (name: string): string => name.trim().replace(/[^\p{L}\p{N}._-]+/gu, '-').replace(/^-+|-+$/g, '') || 'workflow';
