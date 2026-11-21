// The shapes this UI reads, in one file (docs/v2-ui-brief.md): wisp-threads-api's, as its ts-rs bindings write them
// (plugins/threads/api/bindings/threads/*.ts, named on each), and the store rows wisp.threads publishes on a task's
// `session:<id>` (plugins/threads/host/src/store.rs: a row is its columns; `threads_health` keeps the Health under
// `body`). apps/web/test/mock-commands.test.ts checks the mock's answers against the binding files.

/** By.ts */
export type By = 'agent' | 'you';
/** Level.ts */
export type Level = 'ok' | 'heavy' | 'worn';
/** Repeat.ts */
export interface Repeat {
  readonly sig: string;
  readonly n: number;
}
/** Health.ts */
export interface Health {
  readonly task: string;
  readonly level: Level;
  readonly reasons: readonly string[];
  readonly turns: number;
  readonly compactions: number;
  readonly share?: number;
  readonly repeat?: Repeat;
  readonly reverted: number;
  readonly notesAge?: number;
}
/** Section.ts */
export interface Section {
  readonly id: string;
  readonly label: string;
  readonly text: string;
  readonly rev: number;
  readonly by: By;
  readonly at: number;
}
/** Notes.ts */
export interface Notes {
  readonly task: string;
  readonly sections: readonly Section[];
  readonly rev: number;
  readonly seen: number;
  readonly lead?: string;
}
/** Revision.ts */
export interface Revision {
  readonly section: string;
  readonly rev: number;
  readonly text: string;
  readonly by: By;
  readonly at: number;
}
/** Brief.ts: what the fresh-thread brief command answers */
export interface Brief {
  readonly task: string;
  readonly text: string;
  readonly notesRev: number;
  readonly diff?: string;
}
/** Fresh.ts: what starting a fresh thread answers */
export interface Fresh {
  readonly task: string;
  readonly turnId?: string;
  readonly queued?: string;
}

/** AuditStatus.ts: asking, done, or skipped (with why) */
export type AuditStatus = 'running' | 'done' | 'skipped';
/** Audit.ts: what a compaction kept of the goal, constraints, decisions and what failed ("Kept 6 of 8") */
export interface Audit {
  readonly task: string;
  /** the compaction's turn */
  readonly turn: string;
  readonly status: AuditStatus;
  /** why it was skipped */
  readonly why?: string;
  readonly kept: number;
  readonly total: number;
  /** the items it lost, as the Notes say them ("Constraints: No Redis") */
  readonly lost: readonly string[];
  /** told to the agent in a follow-up's note already */
  readonly told: boolean;
  readonly at: number;
}

/** `threads_notes`: one section of a task's Notes (id `<task>|<section>`). */
export interface NoteRow {
  readonly id: string;
  readonly task_id: string;
  readonly section: string;
  readonly text: string;
  readonly rev: number;
  readonly by: By;
  readonly at: number;
}
/** `threads_revisions`: one version of a section (id `<task>|<section>|<rev>`). */
export type RevisionRow = NoteRow;
/** `threads_meta`: the Notes' rev and the rev the agent last read (id: the task). */
export interface MetaRow {
  readonly id: string;
  readonly rev: number;
  readonly seen: number;
}
/** `threads_health`: the thread's health as last worked out (id: the task). */
export interface HealthRow {
  readonly id: string;
  readonly body: Health;
}

/** `threads_audit`: one compaction's audit (id `<task>|<turn>`), the Audit under `body`. */
export interface AuditRow {
  readonly id: string;
  readonly task_id: string;
  readonly at: number;
  readonly body: Audit;
}

export const KIND = {
  audits: 'threads_audit',
  notes: 'threads_notes',
  revisions: 'threads_revisions',
  meta: 'threads_meta',
  health: 'threads_health',
  session: 'session',
} as const;

/** The plugin's id, and the overlay view that shows the Thread from the task's menu (a sheet on a phone). */
export const PLUGIN = 'wisp.threads';
export const THREAD_VIEW = 'threads.thread';
