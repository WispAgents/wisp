// Long threads on the mock (ADR-0079, plugins/threads): a task's Notes, revisions, meta and health as the store
// publishes them on its `session:<id>` (plugins/threads/host/src/store.rs: `threads_notes` `<task>|<section>`,
// `threads_revisions` `<task>|<section>|<rev>`, `threads_meta` and `threads_health` by task, its Health under
// `body`), and the commands the Thread tab sends, answered with the api's shapes (plugins/threads/api/bindings/threads:
// Notes, Section, Revision, Health, Brief, Fresh; test/mock-commands.test.ts checks the fields). The seeded thread is
// "Add rate limiting to /login" (s0002): Worn, three sections written, two by you.

import type { EntityOp, Topic } from '@wisp/protocol';

interface Ctx {
  readonly peer: string;
  now(): number;
  rows(topic: Topic): readonly { e: string; id: string; v: unknown }[];
}
type Fn = (args: unknown, ctx: Ctx) => { ops: EntityOp[]; result?: unknown };
type Row = { e: string; id: string; v: unknown };

/** wisp-threads-core notes::META: the order, the labels, which sections are one text. */
const META = [
  ['goal', 'Goal', true],
  ['constraints', 'Constraints', false],
  ['decisions', 'Decisions', false],
  ['failed', 'Tried and failed', false],
  ['state', 'Where it stands', true],
  ['open', 'Open questions', false],
  ['files', 'Key files', false],
] as const;

interface Note {
  id: string;
  task_id: string;
  section: string;
  text: string;
  rev: number;
  by: 'agent' | 'you';
  at: number;
}

export const THREAD_TASK = 's0002';
const MIN = 60_000;

/** The seeded thread's rows, on `session:s0002`. */
export function threadRows(now: number, task = THREAD_TASK): Row[] {
  const n = (section: string, text: string, rev: number, by: Note['by'], ago: number): Note => ({ id: `${task}|${section}`, task_id: task, section, text, rev, by, at: now - ago });
  const notes = [
    n('goal', 'Rate-limit /login: 5 attempts per minute per IP, then 429 with Retry-After.', 2, 'you', 62 * MIN),
    n('constraints', 'No new dependencies\nKeep the public API of auth/login\nRead the IP from X-Forwarded-For behind the proxy', 1, 'you', 61 * MIN),
    n('decisions', 'Sliding window over fixed buckets: no burst at the minute edge\nIn-memory store: one instance today', 3, 'agent', 4 * MIN),
    n('failed', 'express-rate-limit: a new dependency (constraint)', 1, 'agent', 30 * MIN),
    n('state', 'Limiter and route done; the window-edge test fails 1 in 10 runs.', 4, 'agent', 2 * MIN),
  ];
  const revs = [
    { ...n('goal', 'Rate-limit /login.', 1, 'agent', 90 * MIN), id: `${task}|goal|1` },
    { ...notes[0], id: `${task}|goal|2` },
    { ...n('state', 'Reading the route.', 1, 'agent', 50 * MIN), id: `${task}|state|1` },
    { ...n('state', 'Limiter written, no tests yet.', 2, 'agent', 25 * MIN), id: `${task}|state|2` },
    { ...n('state', 'Tests pass but the edge one is flaky.', 3, 'agent', 9 * MIN), id: `${task}|state|3` },
    { ...notes[4], id: `${task}|state|4` },
  ];
  const health = { task, level: 'worn', reasons: ['context 85% full', 'compacted 3×', 'the same error 4× in the last 3 turns'], turns: 41, compactions: 3, share: 0.85, repeat: { sig: 'expected 429 got 200', n: 4 }, reverted: 0, notesAge: 1 };
  return [
    ...notes.map((v) => ({ e: 'threads_notes', id: v.id, v })),
    ...revs.map((v) => ({ e: 'threads_revisions', id: v.id, v })),
    { e: 'threads_meta', id: task, v: { id: task, rev: 11, seen: 10, turn: 40, count: 41, at: now - 2 * MIN } },
    { e: 'threads_health', id: task, v: { id: task, body: health } },
  ];
}

const bare = (id: string): string => id.slice(id.indexOf(':') + 1);
const topicOf = (task: string): Topic => `session:${task}` as Topic;
const args = (a: unknown): Record<string, unknown> => (typeof a === 'object' && a !== null ? (a as Record<string, unknown>) : {});
const taskOf = (a: unknown): string => {
  const id = args(a)['id'];
  if (typeof id !== 'string' || id === '') throw { code: 'invalid', message: 'id is required' };
  return bare(id);
};
const of = <T>(ctx: Ctx, task: string, e: string): T[] =>
  ctx
    .rows(topicOf(task))
    .filter((r) => r.e === e)
    .map((r) => r.v as T);

/** Notes.ts: every section in order (empty ones too), the rev and what the agent last read. */
function notes(ctx: Ctx, task: string, over: Note[] = []): unknown {
  const rows = [...over, ...of<Note>(ctx, task, 'threads_notes').filter((r) => r.task_id === task)];
  const meta = of<{ rev: number; seen: number }>(ctx, task, 'threads_meta')[0];
  return {
    task,
    sections: META.map(([id, label]) => {
      const r = rows.find((x) => x.section === id);
      return { id, label, text: r?.text ?? '', rev: r?.rev ?? 0, by: r?.by ?? 'agent', at: r?.at ?? 0 };
    }),
    rev: (meta?.rev ?? 0) + (over.length ? 1 : 0),
    seen: meta?.seen ?? 0,
  };
}

/** notes.rs clean, enough for the mock: lists lose bullets and repeats (case and punctuation ignored). */
function clean(section: string, text: string): string {
  const one = META.find((m) => m[0] === section)?.[2];
  if (one) return text.trim();
  const word = (x: string): string => x.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').trim(); // "No Redis." is "no redis"
  const out: string[] = [];
  for (const l of text.split('\n').map((x) => x.trim().replace(/^(?:[-*•]|\d{1,3}[.)])\s+/, ''))) if (l && !out.some((y) => word(y) === word(l))) out.push(l);
  return out.slice(-20).join('\n');
}

export const THREAD_COMMANDS: Record<string, Fn> = {
  'threads.notes': (a, ctx) => ({ ops: [], result: notes(ctx, taskOf(a)) }),
  // rewrites one section whole: its row, a revision, the Notes' rev; the same text again writes nothing
  'threads.notesUpdate': (a, ctx) => {
    const task = taskOf(a);
    const x = args(a);
    const section = String(x['section'] ?? '');
    if (!META.some((m) => m[0] === section)) throw { code: 'invalid', message: `no section "${section}" (one of ${META.map((m) => m[0]).join(', ')})` };
    const text = clean(section, String(x['text'] ?? ''));
    const was = of<Note>(ctx, task, 'threads_notes').find((r) => r.section === section);
    if (was && was.text === text) return { ops: [], result: notes(ctx, task) };
    const by = x['by'] === 'agent' ? 'agent' : 'you';
    const row: Note = { id: `${task}|${section}`, task_id: task, section, text, rev: (was?.rev ?? 0) + 1, by, at: ctx.now() };
    const meta = of<{ id: string; rev: number; seen: number; turn: number; count: number; at: number }>(ctx, task, 'threads_meta')[0] ?? { id: task, rev: 0, seen: 0, turn: 0, count: 0, at: 0 };
    const t = topicOf(task);
    return {
      ops: [
        { topic: t, e: 'threads_notes', id: row.id, op: 'upsert', v: row },
        { topic: t, e: 'threads_revisions', id: `${row.id}|${row.rev}`, op: 'upsert', v: { ...row, id: `${row.id}|${row.rev}` } },
        { topic: t, e: 'threads_meta', id: task, op: 'upsert', v: { ...meta, rev: meta.rev + 1, at: ctx.now() } },
      ],
      result: notes(ctx, task, [row]),
    };
  },
  'threads.revisions': (a, ctx) => {
    const task = taskOf(a);
    const section = args(a)['section'];
    const all = of<Note>(ctx, task, 'threads_revisions').filter((r) => r.task_id === task && (section === undefined || r.section === section));
    return { ops: [], result: all.sort((p, q) => q.at - p.at).map(({ section: s, rev, text, by, at }) => ({ section: s, rev, text, by, at })) };
  },
  'threads.health': (a, ctx) => {
    const task = taskOf(a);
    const kept = of<{ body: unknown }>(ctx, task, 'threads_health')[0]?.body;
    return { ops: [], result: kept ?? { task, level: 'ok', reasons: [], turns: 1, compactions: 0, reverted: 0 } };
  },
  // Brief.ts: the Notes as markdown and the folder's recent diff
  'threads.freshBrief': (a, ctx) => {
    const task = taskOf(a);
    const n = notes(ctx, task) as { rev: number; sections: { label: string; text: string; id: string }[] };
    const md = n.sections
      .filter((s) => s.text)
      .map((s) => `### ${s.label}\n${META.find((m) => m[0] === s.id)?.[2] ? s.text : s.text.split('\n').map((l) => `- ${l}`).join('\n')}`)
      .join('\n\n');
    const diff = ' src/auth/limiter.ts | 48 ++++++++\n src/routes/login.ts |  6 +-\n 2 files changed, 52 insertions(+), 2 deletions(-)';
    const text = `# Fresh thread\n\nThis task carries on in a new conversation: the old one got long. Everything that matters from it is below.\n\n## Task notes\n\n${md}\n\n## The work\n\n\`\`\`\n${diff}\n\`\`\``;
    return { ops: [], result: { task, text, notesRev: n.rev, diff } };
  },
  // Fresh.ts: the new conversation's first turn; health counts from it (the row back to Fine)
  'threads.fresh': (a, ctx) => {
    const task = taskOf(a);
    const t = topicOf(task);
    return {
      ops: [{ topic: t, e: 'threads_health', id: task, op: 'upsert', v: { id: task, body: { task, level: 'ok', reasons: [], turns: 0, compactions: 0, reverted: 0 } } }],
      result: { task, turnId: `${task}:fresh-${ctx.now()}` },
    };
  },
};
