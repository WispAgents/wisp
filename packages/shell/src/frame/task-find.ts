// Find in a task (R4-3): what the words match, as data. It reads the step rows the transcripts are drawn from (the
// same rows, so a hit names the step its transcript draws), never the page: a step's detail and a folded cluster's
// steps aren't in the page until opened. A step's output is read as the step shows it (stepOutput): a .env file's
// masked text, its value only once Reveal brought it. The turns the window sends as stubs have no rows here: the
// server searches those (`runtime.searchTask`) and its hits stand in for theirs until the turn is opened.
import type { EnvReveal } from '@wisp/ui';
import { stepIdOf } from './step-open.ts';
import { stepOutput, type TranscriptStepRow } from './TurnTranscript.tsx';

/** How a turn's .env outputs show: masked unless that step was revealed (env-reveal.ts). */
export interface FindEnv {
  on(): boolean;
  shown(id: string): boolean;
  raw(id: string): string | undefined;
}

/** A turn's Reveal (env-reveal.ts) as Find and Copy read it: the revealed output's text. */
export function findEnvOf(env: EnvReveal): FindEnv {
  return {
    on: env.on,
    shown: env.shown,
    raw: (id) => {
      const r = env.raw?.(id);
      const v = r?.['output'] ?? r?.['text'];
      return typeof v === 'string' ? v : undefined;
    },
  };
}

export interface FindTurn {
  readonly turnId: string;
  /** the window sent it as a stub: no rows, the server's hits count for it */
  readonly stub: boolean;
  readonly steps: readonly TranscriptStepRow[];
  readonly env?: FindEnv;
}

export interface FindHit {
  readonly turnId: string;
  /** the step's id in its turn (step-open.ts); a server hit has its event's seq only */
  readonly stepId?: string;
  readonly seq?: number;
  readonly snippet?: string;
}

/** What a step says that Find reads, as its transcript shows it. */
export function stepTexts(a: TranscriptStepRow, id: string, env?: FindEnv): string[] {
  if (a.kind === 'text') return [a.label ?? ''];
  if (a.kind === 'thinking') return [a.out ?? a.label ?? ''];
  const out = stepOutput(a, env?.on() ?? true, env?.shown(id) ?? false, env?.raw(id));
  return [a.label ?? '', a.title ?? '', a.detail ?? '', out?.text ?? ''];
}

/** The steps of the loaded turns that hold `query` (case-insensitive), in order: one hit per step. */
export function findInTurns(turns: readonly FindTurn[], query: string): FindHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: FindHit[] = [];
  for (const t of turns) {
    if (t.stub) continue;
    t.steps.forEach((a, i) => {
      const id = stepIdOf(a, i);
      if (stepTexts(a, id, t.env).some((s) => s.toLowerCase().includes(q))) out.push({ turnId: t.turnId, stepId: id, ...(a.seq !== undefined && { seq: a.seq }) });
    });
  }
  return out;
}

/** Every hit in the task's order: the loaded turns' own, and the server's for the turns still stubs. */
export function mergeHits(turns: readonly FindTurn[], local: readonly FindHit[], remote: readonly FindHit[]): FindHit[] {
  const stubs = new Set(turns.filter((t) => t.stub).map((t) => t.turnId));
  const of = new Map<string, FindHit[]>();
  for (const h of [...local, ...remote.filter((r) => stubs.has(r.turnId))]) of.set(h.turnId, [...(of.get(h.turnId) ?? []), h]);
  return turns.flatMap((t) => of.get(t.turnId) ?? []);
}

/** Where to go once a stubbed turn's hit landed whole: its step by the same event, else the turn's first hit. */
export function landed(hits: readonly FindHit[], was: FindHit): number {
  const i = hits.findIndex((h) => h.turnId === was.turnId && h.stepId !== undefined && was.seq !== undefined && h.seq === was.seq);
  return i >= 0 ? i : hits.findIndex((h) => h.turnId === was.turnId && h.stepId !== undefined);
}
