// A task's turn rows, as the ledger reads them (ADR-0068): the hero that leads the row, what came of it, whether wisp
// sent it (and what kind of notice), the paths it changed and whether it shipped. A whole turn carries its blocks and
// its changes; a stub (plugins/runtime/core/src/turn_window.rs) keeps its hero, `files` and `shipped` instead.
import type { LedgerTurn, Outcome } from '@wisp/ui/pure';

export interface TurnLike {
  readonly id: string;
  readonly idx: number;
  readonly prompt: string;
  readonly started_at: number;
  readonly status?: string;
  readonly body: {
    readonly by?: string;
    readonly isError?: boolean;
    readonly changedFiles?: number;
    readonly refusal?: unknown;
    readonly files?: readonly string[];
    readonly shipped?: boolean;
    readonly changes?: readonly { readonly file: string }[];
    readonly ext?: { readonly 'wisp.runtime'?: { readonly changes?: readonly { readonly file: string }[] } };
    readonly presentation?: { readonly blocks?: readonly { readonly type: string; readonly title?: string; readonly status?: string; readonly state?: string; readonly items?: readonly unknown[] }[] };
  };
}

/** Who sent a turn when it wasn't the user (`turn.by`): these are the user's own (a workflow's step is their prompt). */
const YOURS = new Set(['user', 'you', 'step']);

/** Whether wisp (or something on its side: a schedule, the crew, a lead) sent the turn. */
export const isNotice = (by: string | undefined): by is string => by !== undefined && by !== '' && !YOURS.has(by);

/** The senders with a word of their own (`shell.ledger.by.<by>`); others read as their name. */
export const NOTICE_KINDS = ['later', 'crew', 'lead', 'agent', 'channel', 'compact'] as const;

/** A wisp notice's first clause, as its kind ("CI finished on the open PR: 7 of 7…" → "CI finished on the open PR"). */
export function clause(prompt: string): string {
  const line = (prompt.trim().split('\n')[0] ?? '').trim();
  const cut = line.search(/[:.!?](\s|$)/);
  const s = (cut > 0 ? line.slice(0, cut) : line).trim();
  return s.length > 48 ? `${s.slice(0, 47).trimEnd()}…` : s;
}

const pathsOf = (b: TurnLike['body']): string[] => {
  if (b.files) return [...b.files];
  const list = b.ext?.['wisp.runtime']?.changes ?? b.changes ?? [];
  return [...new Set(list.map((c) => c.file).filter(Boolean))];
};

/** The same rule as the stub's (`turn_window::shipped`): a commits block with commits, or an open or merged PR. */
export function shippedOf(b: TurnLike['body']): boolean {
  if (b.shipped === true) return true;
  return (b.presentation?.blocks ?? []).some((x) =>
    x.type === 'commits' ? (x.items?.length ?? 0) > 0 : x.type === 'graph' ? x.state === 'open' || x.state === 'merged' : false,
  );
}

export function outcomeOf(status: string | undefined, b: TurnLike['body'], files: number): Outcome {
  if (status === 'running' || status === 'held') return 'running';
  const hero = b.presentation?.blocks?.find((x) => x.type === 'hero');
  if (status === 'error' || b.isError === true || b.refusal !== undefined || hero?.status === 'blocked') return 'failed';
  if (hero?.status === 'info') return 'answer';
  if (hero?.status === 'done' || hero?.status === 'partial') return files > 0 || (b.changedFiles ?? 0) > 0 || shippedOf(b) ? 'done' : 'answer';
  return files > 0 || (b.changedFiles ?? 0) > 0 ? 'done' : 'answer';
}

/** `kind(by)`: a notice's word for its sender (`by` other than 'wisp'); a 'wisp' notice reads as its first clause. */
export function ledgerTurn(row: TurnLike, kind: (by: string) => string): LedgerTurn {
  const b = row.body;
  const files = pathsOf(b);
  const hero = b.presentation?.blocks?.find((x) => x.type === 'hero')?.title ?? '';
  const by = b.by;
  return {
    id: row.id,
    n: row.idx + 1,
    at: row.started_at,
    hero,
    prompt: row.prompt,
    outcome: outcomeOf(row.status, b, files.length),
    ...(isNotice(by) && { notice: by === 'wisp' ? clause(row.prompt) || kind(by) : kind(by) }),
    files,
    shipped: shippedOf(b),
  };
}
