// Save to repo (ADR-0033): a task written into its repo's `.wisp/` (spec, report, transcript, scrubbed). Two ways in:
// the offer under the task's last answer (a repo on `track: ask` sets `ext["wisp.history"].offer` when the task is
// done; Save calls `history.save`), and Save to repo in the task's ⋯ menu for any task (`history.track`). Both share
// one status per task, so the answer says Saving…, then what was saved, whichever was pressed. DOM-free.
import { defineCommand, readonly, signal, type EntityKey } from '@wisp/client';

/** What the task's `wisp.history` slot holds (wisp_history_api::EXT): offered, and what the last save wrote. */
export interface HistoryExt {
  readonly offer?: boolean;
  readonly repo?: string;
  readonly tracked?: { readonly repo?: string; readonly files?: readonly string[]; readonly store?: string };
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
export type SaveState = { readonly s: 'saving' } | { readonly s: 'saved'; readonly files: number; readonly repo: string; readonly scrubbed: number } | { readonly s: 'failed' };

const states = signal<Readonly<Record<string, SaveState>>>({});

/** Every task's save state in this window (a client signal: a component reads it through useSignal). */
export const saveStates = readonly(states);

/** The save state of a task (by its entity key). */
export const saveStateOf = (key: string): SaveState | undefined => states()[key];

function setState(key: string, v: SaveState): void {
  states({ ...states(), [key]: v });
}

/** What the answer shows: the offer (Save to repo), Saving…, Saved, or nothing. */
export type OfferView = { readonly v: 'offer' } | { readonly v: 'saving' } | { readonly v: 'saved'; readonly files: number; readonly scrubbed: number } | { readonly v: 'none' };

/** The line under the last answer, from the task's slot and this window's save. A save from here shows its result
 *  (with how many files and secrets scrubbed); a refused one offers again if the task is still offered. */
export function offerView(ext: HistoryExt, state: SaveState | undefined): OfferView {
  if (state?.s === 'saving') return { v: 'saving' };
  if (state?.s === 'saved') return { v: 'saved', files: state.files, scrubbed: state.scrubbed };
  return ext.offer === true ? { v: 'offer' } : { v: 'none' };
}

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);

interface TurnLike {
  readonly id?: string;
  readonly idx?: number;
  readonly sessionId?: string;
  readonly body?: { readonly id?: string; readonly idx?: number; readonly sessionId?: string };
}

/** The bare id of the task's last turn (highest idx) among the turn rows. */
export function lastTurnOf(rows: readonly unknown[], session: string): string | undefined {
  let best: { id: string; idx: number } | undefined;
  for (const r of rows as readonly TurnLike[]) {
    const sid = r.body?.sessionId ?? r.sessionId;
    if (sid !== session) continue;
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

/** Save to repo from the task's ⋯ menu (any task; again rewrites its files in place). */
export const trackCmd = defineCommand<{ key: EntityKey }, Tracked>({
  id: 'history.track',
  owner: (a) => a.key,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'Saving… under the task’s last answer',
});

/** Runs a save and keeps its state for the answer to show; a second press while one runs does nothing. */
export async function runSave(run: (key: EntityKey) => Promise<Tracked>, key: EntityKey): Promise<void> {
  if (saveStateOf(key)?.s === 'saving') return;
  setState(key, { s: 'saving' });
  try {
    const t = await run(key);
    setState(key, { s: 'saved', files: t.files.length, repo: t.repo, scrubbed: t.scrubbed });
  } catch {
    // refused: the shell's onError toasts why; the offer (if any) comes back
    setState(key, { s: 'failed' });
  }
}
