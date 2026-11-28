// Ship it's writes (v1 routes/ship.js, one command each, so wisp-ship's route adapters map 1:1).
// Each declares what shows at once; a refusal rolls it back and the shell's toast says why. Args
// carry the task's entity key: its peer runs the command (origin routing).

import { defineCommand, type EntityKey, type EntityReader, type LocalOp } from '@wisp/client';
import type { PatchOp } from '@wisp/protocol';
import { shipRow } from './row.ts';
import { SESSION, type MergeResult, type PrSummary, type Review, type ShipInfo, type ShipResult, type ShipSession, type ShipStep } from './types.ts';

interface Task {
  readonly key: EntityKey;
}
const owner = (a: Task): string => a.key;
/** wisp-ship's commands name the task by `session`, its bare id (the UI's args carry the entity key) */
const wire = (a: Task): Record<string, unknown> => {
  const { key, ...rest } = a as Task & Record<string, unknown>;
  return { ...rest, session: key.slice(key.indexOf(':') + 1) };
};
const raw = (base: EntityReader, key: EntityKey): { id: string; summary?: unknown } | undefined => base.peek<{ id: string; summary?: unknown }>(SESSION, key) as never;
const row = (base: EntityReader, key: EntityKey): ShipSession | undefined => shipRow(raw(base, key), key);
/** A field of the row where the server keeps it: under `summary` on a synced row (flat on a flat one). */
const set = (base: EntityReader, key: EntityKey, path: string, value: unknown): LocalOp => {
  const at = raw(base, key)?.summary !== undefined ? `/summary${path}` : path;
  return { e: SESSION, id: key, op: 'patch', ops: [{ op: 'add', path: at, value } satisfies PatchOp] };
};

/** GET /sessions/:id/ship: what would ship (routes, counts, the drafted message). Reads only. */
export const prepare = defineCommand<Task, ShipInfo>({ id: 'ship.view', owner, wire, optimistic: 'none', pending: 'the ship.prepare control' });

export interface ShipArgs extends Task {
  readonly title: string;
  readonly body: string;
  readonly draft: boolean;
  /** save the task into `.wisp/` before committing (when ticked in Ship it) */
  readonly saveTask?: boolean;
  /** Keep it green (ADR-0132): the Steward for the PR, set once it's there */
  readonly steward?: 'fix' | 'review' | 'off';
  /** the dialog's view: it pushes to open PRs (no new one) */
  readonly update: boolean;
  /** the first stage, so the row's track starts there at once */
  readonly first: ShipStep;
}

/** POST /sessions/:id/ship: commit, push, open or update the PR. The row starts its track at once. */
export const ship = defineCommand<ShipArgs, ShipResult>({
  id: 'ship.ship',
  owner, wire,
  optimistic: (base, a) => (row(base, a.key) ? [set(base, a.key, '/shipping', { step: a.first, update: a.update })] : []),
});

/** TRY-13: the task's own agent writes the PR's title and description (a side run: its button is pending meanwhile). */
export const write = defineCommand<Task, { readonly title: string; readonly body: string }>({ id: 'ship.write', owner, wire, optimistic: 'none', pending: 'the ship.write control' });

/** POST /sessions/:id/pr: check the PR now (wisp also checks every minute). */
export const refresh = defineCommand<Task, PrSummary>({ id: 'ship.refresh', owner, wire, optimistic: 'none', pending: 'the ship.refresh control' });

/** The Steward on or off (ADR-0132): the PR row's Stop / Keep green. The row's line changes at once. */
export const setSteward = defineCommand<Task & { readonly mode: 'fix' | 'review' | 'off' }>({
  id: 'ship.steward',
  owner, wire,
  optimistic: (base, a) => {
    const s = row(base, a.key);
    if (!s) return [];
    const was = s.steward;
    const value = a.mode === 'off' ? null : { ...was, mode: a.mode, step: was && was.step !== 'stopped' ? was.step : 'watching', why: '' };
    return [set(base, a.key, '/steward', value)];
  },
});

/** POST /sessions/:id/fix-conflicts: the row says "Fixing merge conflicts" at once. */
export const fixConflicts = defineCommand<Task>({
  id: 'ship.fix_conflicts',
  owner, wire,
  optimistic: (base, a) => {
    const s = row(base, a.key);
    return s ? [set(base, a.key, '/conflictFix', { ...s.conflictFix, step: 'fixing', why: '' })] : [];
  },
});

/** POST /sessions/:id/review: review by another harness, now. The chip says Reviewing at once. wisp.review
 * runs it (`review.maybe`, forced: by hand it runs whatever the setting says). */
export const review = defineCommand<Task>({
  id: 'review.maybe',
  owner,
  wire: (a) => ({ id: a.key.slice(a.key.indexOf(':') + 1), force: true }),
  optimistic: (base, a) => {
    const s = row(base, a.key);
    if (!s?.pr) return [];
    const was = s.review;
    const head = s.pr.head ?? was?.head;
    const next: Review = { ...was, status: 'running', current: true, findings: [], ...(head !== undefined && { head }), ...(was?.by && { by: was.by }) };
    return [set(base, a.key, '/review', next)];
  },
});

/** POST /sessions/:id/merge: squash-merge each open PR, sync the local base, clean up. The confirm
 * came first; the merge itself takes seconds, so the flow shows it (not a guess at the result). `admin` bypasses the
 * base branch's rules (`gh pr merge --admin`), asked after a `merge_blocked` refusal. */
export const merge = defineCommand<Task & { readonly admin: boolean }, MergeResult>({
  id: 'ship.merge',
  owner,
  wire,
  optimistic: 'none',
  pending: 'the ship.merge control',
  // the merge flow says why itself: a branch-rules hold opens the bypass confirm, not a toast of gh's advice
  quiet: true,
});

/** GET /sessions/:id/check-log: the end of a failing check's log, for Ask to fix. Reads only. */
export const checkLog = defineCommand<Task & { readonly url: string }, { log: string }>({ id: 'ship.check_log', owner, wire, optimistic: 'none', pending: 'the ship.checkLog control' });

export const COMMANDS = [prepare, ship, refresh, fixConflicts, review, merge, checkLog] as const;
