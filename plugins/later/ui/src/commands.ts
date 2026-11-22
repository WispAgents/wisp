// Later's writes (ids from the reference contract's COMMANDS, v1's /later routes). Each declares the
// change it shows at once; the client rolls it back with a red toast when the peer refuses. Ids in
// args are entity keys (`<peer>:<id>`): the key routes the command to the peer that owns the row.

import { defineCommand, type EntityOps, type EntityReader, type LocalOp } from '@wisp/client';
import type { PatchOp } from '@wisp/protocol';
import { sortLater } from './rules.ts';
import { KIND, type Held, type LaterView, type Plan, type SessionRow } from './types.ts';

type Key = Parameters<EntityReader['peek']>[1];
const asKey = (s: string): Key => s as Key;
/** `<peer>:<id>` → `<id>`: the key routes the command, the server reads its own id. */
const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
const bareSession = <A extends { sessionId: string }>(a: A): A => ({ ...a, sessionId: bare(a.sessionId) });
const bareId = <A extends { id: string }>(a: A): A => ({ ...a, id: bare(a.id) });

/** A patch setting (or removing, when undefined) top-level fields of a row the reader holds. */
function patchRow<T extends object>(base: EntityReader, kind: string, key: string, next: (row: T) => Partial<Record<keyof T, unknown>>): EntityOps {
  const row = base.peek<T>(kind, asKey(key)) as T | undefined;
  if (!row) return [];
  const ops: PatchOp[] = [];
  for (const [k, v] of Object.entries(next(row))) {
    if (v === undefined) {
      if (k in row) ops.push({ op: 'remove', path: `/${k}` });
    } else ops.push({ op: 'add', path: `/${k}`, value: v });
  }
  return ops.length ? [{ e: kind, id: asKey(key), op: 'patch', ops } satisfies LocalOp] : [];
}

const session = (base: EntityReader, key: string, next: (row: SessionRow) => Partial<Record<keyof SessionRow, unknown>>): EntityOps => patchRow<SessionRow>(base, KIND.session, key, next);
const listOrNone = (list: LaterView[]): LaterView[] | undefined => (list.length ? list : undefined);
type ExtRow = SessionRow & { body?: { ext?: Record<string, { later?: LaterView[] } | undefined> } };
/** What's scheduled on a task: the flat list (v1's row, an earlier patch), else wisp.later's mirror in the server
 * row's `body.ext` (LaterFoot reads the same). */
export const laterOf = (s: SessionRow): LaterView[] => s.later ?? (s as ExtRow).body?.ext?.['wisp.later']?.later ?? [];
/** The list after a change: emptied, it stays as `[]` when it came from the mirror (removing a flat list that isn't
 * there would leave the mirror showing what was just taken away). */
const after = (s: SessionRow, list: LaterView[]): LaterView[] | undefined => (s.later ? listOrNone(list) : list);

export interface AddArgs {
  sessionId: string;
  at: number;
  text: string;
  remind?: boolean;
  /** the optimistic row's id and time, so a retry with the same args is the same run */
  tmp: string;
  setAt: number;
}
export const addFollowUp = defineCommand<AddArgs, LaterView>({
  id: 'later.add',
  owner: (a) => a.sessionId,
  wire: bareSession,
  optimistic: (base, a) =>
    session(base, a.sessionId, (s) => ({
      later: sortLater([...laterOf(s), { id: a.tmp, at: a.at, text: a.text, by: 'user', setAt: a.setAt, ...(a.remind && { remind: true as const }) }]),
    })),
});

export type EditArgs = { sessionId: string; item: string } & ({ at: number } | { text: string } | { now: true });
export const editFollowUp = defineCommand<EditArgs>({
  id: 'later.edit',
  owner: (a) => a.sessionId,
  wire: bareSession,
  optimistic: (base, a) =>
    session(base, a.sessionId, (s) => {
      const list = laterOf(s);
      if ('now' in a) return { later: after(s, list.filter((l) => l.id !== a.item)) };
      if ('text' in a) return { later: list.map((l) => (l.id === a.item ? { ...l, text: a.text } : l)) };
      return { later: sortLater(list.map((l) => (l.id === a.item ? { ...l, at: a.at } : l))) };
    }),
});

/** What an edit in place sends: the new text, or nothing when it was left, emptied or unchanged. */
export function editedText(was: string, text: string | undefined): string | undefined {
  const next = text?.trim();
  return next && next !== was.trim() ? next : undefined;
}

/** Snooze… a reminder that came due: it goes back to Scheduled at that time. */
export const snoozeReminder = defineCommand<{ sessionId: string; at: number; tmp: string; setAt: number }>({
  id: 'later.edit',
  owner: (a) => a.sessionId,
  wire: bareSession,
  dedupe: (a) => `snooze:${a.sessionId}:${a.at}`,
  optimistic: (base, a) =>
    session(base, a.sessionId, (s) => ({
      reminder: undefined,
      unread: false,
      later: sortLater([...laterOf(s), { id: a.tmp, at: a.at, text: s.reminder?.text ?? '', by: 'user', setAt: a.setAt, remind: true as const }]),
    })),
});

export const cancelFollowUp = defineCommand<{ sessionId: string; item: string }>({
  id: 'later.cancel',
  owner: (a) => a.sessionId,
  wire: bareSession,
  optimistic: (base, a) => session(base, a.sessionId, (s) => ({ later: after(s, laterOf(s).filter((l) => l.id !== a.item)) })),
});

/** Save for later / Unsave. at: when it was saved (Undo puts back the old time); state: Undo after Mark done. */
export const park = defineCommand<{ sessionId: string; forLater: boolean; at: number; state?: string }>({
  id: 'later.park',
  owner: (a) => a.sessionId,
  wire: (a) => ({ sessionId: bare(a.sessionId), forLater: a.forLater ? a.at || true : false, ...(a.state && { state: a.state }) }),
  optimistic: (base, a) =>
    session(base, a.sessionId, () => ({
      forLater: a.forLater ? { at: a.at, by: 'user' } : undefined,
      ...(a.state && { state: a.state }),
    })),
});

export interface PlanArgs {
  /** the peer that keeps the plan: this client's own wisp */
  peer: string;
  tmp: string;
  setAt: number;
  kind: 'task' | 'reminder';
  at?: number;
  draft?: true;
  after?: { id: string; title: string };
  body?: NonNullable<Plan['body']>;
  text?: string;
}
export const addPlan = defineCommand<PlanArgs, Plan>({
  id: 'later.plan',
  owner: (a) => a.peer,
  optimistic: (_base, a) => {
    const { peer, tmp, ...rest } = a;
    const title = a.body ? (a.body.prompt.split('\n')[0] ?? '').slice(0, 80) : undefined;
    const v: Plan = { ...rest, id: tmp, ...(title !== undefined && { title }) };
    return [{ e: KIND.plan, id: asKey(`${peer}:${tmp}`), op: 'upsert', v }];
  },
});

export type EditPlanArgs = { id: string } & ({ at: number } | { text: string } | { prompt: string });
export const editPlan = defineCommand<EditPlanArgs>({
  id: 'later.editPlan',
  owner: (a) => a.id,
  wire: bareId,
  optimistic: (base, a) => {
    const raw = base.peek<Plan & { body?: Plan }>(KIND.plan, asKey(a.id));
    if (!raw) return [];
    const enveloped = typeof raw.body === 'object' && raw.body !== null && 'kind' in raw.body;
    const p = enveloped ? (raw.body as Plan) : (raw as Plan);
    const prefix = enveloped ? '/body' : '';
    const patchFields: Partial<Record<keyof Plan, unknown>> =
      'at' in a
        ? { at: a.at, draft: undefined, after: undefined, lost: undefined }
        : 'text' in a
          ? { text: a.text }
          : { body: { ...p.body, prompt: a.prompt }, title: (a.prompt.split('\n')[0] ?? '').slice(0, 80) };
    const ops: PatchOp[] = [];
    for (const [k, v] of Object.entries(patchFields)) {
      if (v === undefined) {
        if (k in p) ops.push({ op: 'remove', path: `${prefix}/${k}` });
      } else ops.push({ op: 'add', path: `${prefix}/${k}`, value: v });
    }
    return ops.length ? [{ e: KIND.plan, id: asKey(a.id), op: 'patch', ops } satisfies LocalOp] : [];
  },
});

/** Start now: the plan goes; its task arrives on the list topic. */
export const startPlan = defineCommand<{ id: string; now: true }, { session?: { id: string } }>({
  id: 'later.editPlan',
  owner: (a) => a.id,
  wire: bareId,
  optimistic: (_base, a) => [{ e: KIND.plan, id: asKey(a.id), op: 'del' }],
});

export const cancelPlan = defineCommand<{ id: string }>({
  id: 'later.cancelPlan',
  owner: (a) => a.id,
  wire: bareId,
  optimistic: (_base, a) => [{ e: KIND.plan, id: asKey(a.id), op: 'del' }],
});

/** A schedule saved for something another plugin owns (a member's check-ins, a trigger): its own command. */
export function applySchedule(cmd: string): ReturnType<typeof defineCommand<Record<string, unknown>>> {
  return defineCommand<Record<string, unknown>>({ id: cmd, owner: (a) => String(a['id'] ?? a['peer'] ?? ''), optimistic: 'none', pending: `the ${cmd} control` });
}

// a held run's Start / Skip are the task page's own card (runtime.release, a person's yes); Later sends none: its old
// held command sent `start` for Wait too (QA r1)
export const COMMANDS = [addFollowUp, editFollowUp, snoozeReminder, cancelFollowUp, park, addPlan, editPlan, startPlan, cancelPlan];
