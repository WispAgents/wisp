// Save to repo (ADR-0033): shared save state and `history.track` for the task ⋯ menu (later.ts) and OfferFoot.
import { defineCommand, readonly, signal, type EntityKey } from '@wisp/client';
// its `history.save` command id is also the lazy half's "Save to repo" key, and only lazy modules (later.ts,
// OfferFoot) import this one: the loader comes with it, so the words are there wherever the id is
import './i18n/lazy.ts';

/** What the task's `wisp.history` slot holds (wisp_history_api::EXT): offered, and what the last save wrote. */
export interface HistoryExt {
  readonly offer?: boolean;
  readonly repo?: string;
  readonly tracked?: { readonly repo?: string; readonly files?: readonly string[]; readonly store?: string };
  readonly pushed?: { readonly branch?: string; readonly commit?: string; readonly pr?: string | null };
}

/** What `history.push` answers (wisp_history_api::Pushed): the branch pushed ('' when already on the base), its PR. */
export interface Pushed {
  readonly branch: string;
  readonly commit: string;
  readonly files: readonly string[];
  readonly pr?: string;
  readonly removed?: readonly string[];
}

/** What `history.track` / `history.save` answer (wisp_history_api::Tracked). */
export interface Tracked {
  readonly repo: string;
  readonly store: string;
  readonly files: readonly string[];
  readonly pr?: string;
  readonly commits: readonly string[];
  readonly scrubbed: number;
}

/** A synced task row: runtime's columns, its `body` (with `ext`), the fields plugins add under `summary`. */
export interface TaskRow {
  readonly id?: string;
  readonly pending?: boolean;
  readonly parent_id?: string | null;
  readonly parentId?: string | null;
  readonly ext?: unknown;
  readonly body?: { readonly ext?: unknown; readonly parent_id?: string | null } | null;
}

const obj = (v: unknown): Record<string, unknown> | undefined => (v && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : undefined);
const bare = (k: string): string => k.slice(k.indexOf(':') + 1);

/** The task's `wisp.history` slot, wherever the row carries `ext` (the synced row's body, or a flat row). */
export function historyOf(row: TaskRow | undefined): HistoryExt {
  const ext = obj(row?.body?.ext) ?? obj(row?.ext);
  return (obj(ext?.['wisp.history']) ?? {}) as HistoryExt;
}

/** A task the ⋯ menu can save: there, not still being created, and not a subagent's (its lead's task holds it). */
export function saveable(row: TaskRow | undefined): boolean {
  if (!row || row.pending) return false;
  return !(row.parent_id ?? row.parentId ?? row.body?.parent_id);
}

/** Where a task's save stands in this window: pressing, done (what it wrote), or refused (the shell's toast says why). */
export type SaveState =
  | { readonly s: 'saving' }
  | { readonly s: 'saved'; readonly files: number; readonly repo: string; readonly scrubbed: number; readonly at: number }
  | { readonly s: 'failed' };

const states = signal<Readonly<Record<string, SaveState>>>({});

/** Every task's save state in this window (a client signal: a component reads it through useSignal). */
export const saveStates = readonly(states);

/** The save state of a task (by its entity key). */
export const saveStateOf = (key: string): SaveState | undefined => states()[key];

function setState(key: string, v: SaveState): void {
  states({ ...states(), [key]: v });
}

/** Save to repo from the task's ⋯ menu (any task; again rewrites its files in place). */
export const trackCmd = defineCommand<{ key: EntityKey }, Tracked>({
  id: 'history.track',
  owner: (a) => a.key,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'Saving… under the task’s last answer',
});

/** Runs a save and keeps its state for the answer to show; a second press while one runs does nothing. */
export async function runSave(run: (key: EntityKey) => Promise<Tracked>, key: EntityKey): Promise<Tracked | undefined> {
  if (saveStateOf(key)?.s === 'saving') return undefined;
  setState(key, { s: 'saving' });
  try {
    const t = await run(key);
    setState(key, { s: 'saved', files: t.files.length, repo: t.repo, scrubbed: t.scrubbed, at: Date.now() });
    return t;
  } catch {
    setState(key, { s: 'failed' });
    return undefined;
  }
}

/** What the answer shows: the offer (Save to repo), Saving…, Saved, or nothing. */
export type OfferView =
  | { readonly v: 'offer' }
  | { readonly v: 'saving' }
  | { readonly v: 'saved'; readonly files: number; readonly scrubbed: number; readonly at: number }
  | { readonly v: 'none' };

/** The line under the last answer, from the task's slot and this window's save. */
export function offerView(ext: HistoryExt, state: SaveState | undefined): OfferView {
  if (state?.s === 'saving') return { v: 'saving' };
  if (state?.s === 'saved') return { v: 'saved', files: state.files, scrubbed: state.scrubbed, at: state.at };
  return ext.offer === true ? { v: 'offer' } : { v: 'none' };
}

interface TurnLike {
  readonly id?: string;
  readonly idx?: number;
  readonly session_id?: string;
  readonly sessionId?: string;
  readonly body?: { readonly id?: string; readonly idx?: number; readonly session_id?: string; readonly sessionId?: string };
}

/** The bare id of the task's last turn (highest idx) among the turn rows, matching camelCase (`body.sessionId`)
 *  or snake_case (`session_id`) and scoped to `peer` when given. */
export function lastTurnOf(rows: readonly unknown[], session: string, peer?: string): string | undefined {
  let best: { id: string; idx: number } | undefined;
  const target = bare(session);
  for (const r of rows as readonly TurnLike[]) {
    if (peer && r.id && r.id.includes(':') && r.id.slice(0, r.id.indexOf(':')) !== peer) continue;
    const rawSid = r.body?.sessionId ?? r.body?.session_id ?? r.session_id ?? r.sessionId;
    if (!rawSid || bare(rawSid) !== target) continue;
    const id = r.body?.id ?? r.id ?? '';
    const idx = r.body?.idx ?? r.idx ?? 0;
    if (!best || idx > best.idx) best = { id: bare(id), idx };
  }
  return best?.id;
}

/** Save to repo on an offered task (the answer's button). */
export const saveCmd = defineCommand<{ key: EntityKey }, Tracked>({
  id: 'history.save',
  owner: (a) => a.key,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'Save to repo, then the line under the answer',
});

/** Push a saved task's `.wisp/` files: onto its branch while its work is open, else as a PR (`history.push`). */
export const pushCmd = defineCommand<{ key: EntityKey }, Pushed>({
  id: 'history.push',
  owner: (a) => a.key,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'Pushing… on the saved line under the answer',
});

/** Where a task's push stands in this window. */
export type PushState =
  | { readonly s: 'pushing' }
  | { readonly s: 'pushed'; readonly branch: string; readonly pr?: string }
  | { readonly s: 'failed' };

const pushes = signal<Readonly<Record<string, PushState>>>({});

/** Every task's push state in this window. */
export const pushStates = readonly(pushes);

/** The push state of a task (by its entity key). */
export const pushStateOf = (key: string): PushState | undefined => pushes()[key];

/** Runs a push and keeps its state; a second press while one runs does nothing. A refusal leaves the button. */
export async function runPush(run: (key: EntityKey) => Promise<Pushed>, key: EntityKey): Promise<Pushed | undefined> {
  if (pushStateOf(key)?.s === 'pushing') return undefined;
  pushes({ ...pushes(), [key]: { s: 'pushing' } });
  try {
    const p = await run(key);
    pushes({ ...pushes(), [key]: { s: 'pushed', branch: p.branch, ...(p.pr ? { pr: p.pr } : {}) } });
    return p;
  } catch {
    pushes({ ...pushes(), [key]: { s: 'failed' } });
    return undefined;
  }
}

/** A task the ⋯ menu can push: saved to its repo (its slot names the files). */
export function pushable(row: TaskRow | undefined): boolean {
  return saveable(row) && (historyOf(row).tracked?.files?.length ?? 0) > 0;
}
