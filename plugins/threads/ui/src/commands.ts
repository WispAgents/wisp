// wisp.threads' commands as the Thread tab sends them (plugins/threads/api/src/lib.rs; each is also the agent's
// wisp_notes and `wisp notes`). Args carry the task's entity key, so its peer runs them (origin routing); the wire
// gets the bare id the api takes. A rewrite shows at once (its section row, the Notes' rev) and a refusal rolls it
// back with the client's toast.
import { defineCommand, entityKey, ownerOf, type EntityKey, type EntityReader, type LocalOp } from '@wisp/client';
import { cleanText, noteId } from './model.ts';
import { KIND, PLUGIN, type Brief, type Fresh, type Health, type MetaRow, type NoteRow, type Notes } from './types.ts';
import './i18n/lazy.ts';

const bare = (key: string): string => key.slice(key.indexOf(':') + 1);
const owner = (a: { key: EntityKey }): string => a.key;

/** The rows a rewrite of `section` changes, as the server will write them (notes.rs update). */
export function rewriteOps(base: EntityReader, a: { key: EntityKey; section: string; text: string }, now: number): LocalOp[] {
  const peer = ownerOf(a.key);
  const task = bare(a.key);
  const id = entityKey(peer, noteId(task, a.section));
  const was = base.peek<NoteRow>(KIND.notes, id);
  const text = cleanText(a.section, a.text);
  if (was?.text === text) return []; // the same text again writes nothing (no new rev)
  const row: NoteRow = { id: noteId(task, a.section), task_id: task, section: a.section, text, rev: (was?.rev ?? 0) + 1, by: 'you', at: now };
  const ops: LocalOp[] = [{ e: KIND.notes, id, op: 'upsert', v: row }];
  const metaKey = entityKey(peer, task);
  const meta = base.peek<MetaRow>(KIND.meta, metaKey);
  if (meta) ops.push({ e: KIND.meta, id: metaKey, op: 'patch', ops: [{ op: 'replace', path: '/rev', value: meta.rev + 1 }] });
  return ops;
}

/** threads.notesUpdate: rewrite one section whole (never append); '' clears it. */
export const notesUpdate = defineCommand<{ key: EntityKey; section: string; text: string }, Notes>({
  id: 'threads.notesUpdate',
  owner,
  wire: (a) => ({ id: bare(a.key), section: a.section, text: a.text, by: 'you' }),
  optimistic: (base, a) => rewriteOps(base, a, Date.now()),
});

/** threads.notes: a subagent's Notes are its lead's, read through the command (their rows are on the lead's topic). */
export const readNotes = defineCommand<{ key: EntityKey }, Notes>({
  id: 'threads.notes',
  owner,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'the Thread tab',
});

/** threads.health: worked out on the spot when the task has no health row yet. */
export const readHealth = defineCommand<{ key: EntityKey }, Health>({
  id: 'threads.health',
  owner,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'the Thread tab',
});

/** threads.freshBrief: what a fresh thread would start from, to read and edit first. */
export const freshBrief = defineCommand<{ key: EntityKey }, Brief>({
  id: 'threads.freshBrief',
  owner,
  wire: (a) => ({ id: bare(a.key) }),
  optimistic: 'none',
  pending: 'Fresh thread',
  feature: { plugin: PLUGIN, command: 'threads.freshBrief', label: 'Fresh thread' },
});

/** threads.fresh: a new conversation on the same task, from the brief as you left it. */
export const fresh = defineCommand<{ key: EntityKey; brief: string }, Fresh>({
  id: 'threads.fresh',
  owner,
  wire: (a) => ({ id: bare(a.key), brief: a.brief }),
  optimistic: 'none',
  pending: 'Start fresh thread',
  feature: { plugin: PLUGIN, command: 'threads.fresh', label: 'Fresh thread' },
});
