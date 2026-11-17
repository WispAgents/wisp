// Every write the list makes, as a command with its optimistic patch (ADR-0006): the row changes at
// once, the peer that owns the task runs it, and a refusal rolls the patch back with a toast. The
// ids and argument shapes are wisp.runtime's (plugins/runtime/api/src/commands.rs): rename, archive,
// markRead, setState, delete, forLater, pin.

import { defineCommand, type EntityKey, type EntityOps, type EntityReader } from '@wisp/client';
import type { PatchOp } from '@wisp/protocol';
import { KIND, type Session, type SessionRow } from './types.ts';

const RUNTIME = 'wisp.runtime';
const row = (base: EntityReader, id: EntityKey): SessionRow | undefined => base.peek<Session>(KIND.session, id);
const patch = (id: EntityKey, ops: PatchOp[]): EntityOps => (ops.length ? [{ e: KIND.session, id, op: 'patch', ops }] : []);
/** the args as the peer reads them: the task by its bare id (`local:01a…` → `01a…`), not the list's entity key */
const bare = <A extends { id: EntityKey }>(a: A): Omit<A, 'id'> & { id: string } => ({ ...a, id: a.id.slice(a.id.indexOf(':') + 1) });

/** Sets or clears an optional field, with the op JSON Patch needs for what's there now. */
function setField(s: SessionRow | undefined, field: keyof Session, value: unknown): PatchOp[] {
  const has = s !== undefined && s[field] !== undefined;
  if (value === undefined || value === null) return has ? [{ op: 'remove', path: `/${field}` }] : [];
  return [{ op: has ? 'replace' : 'add', path: `/${field}`, value }];
}

/** The same for a field of the list's view, which a server row keeps under `summary` (a flat row: on itself). */
function summaryField(s: SessionRow | undefined, field: keyof Session, value: unknown): PatchOp[] {
  const view = (s as { summary?: unknown } | undefined)?.summary;
  if (typeof view !== 'object' || view === null) return setField(s, field, value);
  const has = (view as Record<string, unknown>)[field] !== undefined;
  if (value === undefined || value === null) return has ? [{ op: 'remove', path: `/summary/${field}` }] : [];
  return [{ op: has ? 'replace' : 'add', path: `/summary/${field}`, value }];
}

export const rename = defineCommand<{ id: EntityKey; title: string }>({
  id: 'runtime.rename',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => (row(base, a.id) ? patch(a.id, [{ op: 'replace', path: '/title', value: a.title }]) : []),
  feature: { plugin: RUNTIME, command: 'runtime.rename', label: 'Rename' },
});

/** Archiving also marks it read (server/routes/sessions.js:327). */
export const archive = defineCommand<{ id: EntityKey; archived: boolean }>({
  id: 'runtime.archive',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => {
    const s = row(base, a.id);
    if (!s) return [];
    return patch(a.id, [...setField(s, 'archived', a.archived), ...(a.archived && s.unread ? setField(s, 'unread', false) : [])]);
  },
  feature: { plugin: RUNTIME, command: 'runtime.archive', label: 'Archive' },
});

export const markRead = defineCommand<{ id: EntityKey; unread: boolean }>({
  id: 'runtime.markRead',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => {
    const s = row(base, a.id);
    return s && !!s.unread !== a.unread ? patch(a.id, setField(s, 'unread', a.unread)) : [];
  },
  feature: { plugin: RUNTIME, command: 'runtime.markRead', label: 'Mark read' },
});

/** Mark done (`done`) and Reopen (`review`). Any state change takes it out of For later (sessions.js:332). */
export const setState = defineCommand<{ id: EntityKey; state: 'done' | 'review' }>({
  id: 'runtime.setState',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => {
    const s = row(base, a.id);
    if (!s) return [];
    return patch(a.id, [{ op: 'replace', path: '/state', value: a.state }, ...setField(s, 'forLater', undefined)]);
  },
  feature: { plugin: RUNTIME, command: 'runtime.setState', label: 'Mark done' },
});

export const remove = defineCommand<{ id: EntityKey }>({
  id: 'runtime.delete',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => (row(base, a.id) ? [{ e: KIND.session, id: a.id, op: 'del' }] : []),
  feature: { plugin: RUNTIME, command: 'runtime.delete', label: 'Delete task' },
});

/** Save for later (`at`: when; Undo of Unsave puts the old time back) or take it out. wisp.runtime keeps it in
 *  ext["wisp.runtime"].forLater and its summary shows it from there (host ops.rs for_later, core summary.rs): the For
 *  later section. (runtime.setExt wrote ext["forLater"], which the summary never reads: the row went back, #12.) */
export const forLater = defineCommand<{ id: EntityKey; forLater: boolean; at?: number }>({
  id: 'runtime.forLater',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => patch(a.id, summaryField(row(base, a.id), 'forLater', a.forLater ? { at: a.at ?? 0, by: 'you' } : undefined)),
  feature: { plugin: RUNTIME, command: 'runtime.forLater', label: 'Save for later' },
});

/** Pin (or unpin) in the list. wisp.runtime keeps it in ext["wisp.runtime"].pinned and its summary shows it from there
 *  (host ops.rs pin, core summary.rs): the Pinned pill. (runtime.setExt wrote ext["pinned"], which the summary never
 *  reads: the pin went back, TRY-17 #30.) */
export const pin = defineCommand<{ id: EntityKey; pinned: boolean }>({
  id: 'runtime.pin',
  owner: (a) => a.id,
  wire: bare,
  optimistic: (base, a) => patch(a.id, summaryField(row(base, a.id), 'pinned', a.pinned ? true : undefined)),
  feature: { plugin: RUNTIME, command: 'runtime.pin', label: 'Pin' },
});

export const COMMANDS = [rename, archive, markRead, setState, remove, forLater, pin] as const;
