// Pure conversion of a turn's event log into inline chat / transcript steps, respecting Chat mode's
// Tool calls in chat (`showAll` | `hideSuccessful` | `hideAll` | `zen`) and Show thinking settings.

import type { ToolCallVisibility } from './chat-parse.ts';
import type { TranscriptStepRow } from './TurnTranscript.tsx';

export interface RawTurnEvent {
  readonly seq: number;
  readonly kind?: string;
  readonly body?: {
    readonly kind?: string;
    readonly tool?: string;
    readonly label?: string;
    readonly title?: string;
    readonly text?: string;
    readonly detail?: string;
    readonly file?: string;
    readonly output?: string;
    readonly callId?: string;
    readonly name?: string;
  };
}

export interface ChatStepOpts {
  readonly chatMode: boolean;
  readonly toolCalls: ToolCallVisibility;
  readonly showThinking: boolean;
  readonly running: boolean;
  /** The final answer's text, or the ways it may be written (the raw answer, its last markdown block): the last
   *  message matching any of them is the answer itself, rendered under the steps, so it isn't shown twice in Chat mode. */
  readonly finalText?: string | readonly (string | undefined)[] | undefined;
  /** Presentation notes (appended in non-chat Transcript mode). */
  readonly notes?: string | undefined;
}

const kindOf = (e: RawTurnEvent): string => e.kind ?? e.body?.kind ?? '';

/** Text compared as words: line ends and runs of spaces don't make two messages different. */
const squash = (s: string): string => s.replace(/\s+/g, ' ').trim();

const firstLine = (s: string): string => {
  const line = s.split('\n').find((l) => l.trim().length > 0)?.trim() ?? '';
  return line.length > 90 ? `${line.slice(0, 89)}…` : line;
};

/** Builds the step rows for a turn's event stream (tool calls, errors, thinking, intermediate text). */
export function buildChatSteps(
  events: readonly RawTurnEvent[],
  fallbackActivity: readonly TranscriptStepRow[],
  opts: ChatStepOpts,
): TranscriptStepRow[] {
  const outputs = new Map<string, { readonly text: string; readonly seq: number }>();
  for (const e of events) {
    if (kindOf(e) === 'tool_output' && e.body?.callId) {
      outputs.set(e.body.callId, { text: e.body.text ?? '', seq: e.seq });
    }
  }
  const relevant = events.filter((e) => {
    const k = kindOf(e);
    return k === 'tool' || k === 'tool_error' || k === 'thinking' || k === 'text';
  });

  if (relevant.length === 0) {
    return filterFallback(fallbackActivity, opts);
  }

  const lastToolSeq = opts.running ? findLastOpenToolSeq(relevant, outputs) : -1;
  const out: TranscriptStepRow[] = [];
  for (const e of relevant) {
    const k = kindOf(e);
    if (k === 'tool_error') {
      // Errors always show in every visibility mode (matches t3code).
      out.push({ kind: 'error', label: e.body?.text ?? e.body?.name ?? '', bad: true });
      continue;
    }
    if (k === 'thinking') {
      const full = (e.body?.text ?? '').trim();
      if (!full || !opts.showThinking) continue;
      if (opts.chatMode && opts.toolCalls === 'zen') continue;
      const summary = firstLine(full) || 'Thought';
      out.push({ kind: 'thinking', label: summary, out: full, seq: e.seq });
      continue;
    }
    if (k === 'text') {
      const msg = (e.body?.text ?? '').trim();
      if (!msg) continue;
      if (!opts.chatMode) continue;
      if (opts.toolCalls === 'zen') continue;
      out.push({ kind: 'text', label: msg, seq: e.seq });
      continue;
    }
    // k === 'tool'
    const o = e.body?.callId ? outputs.get(e.body.callId) : undefined;
    const active = opts.running && e.seq === lastToolSeq && !e.body?.output && !o;
    if (opts.chatMode && !keepTool(opts.toolCalls, active)) continue;
    out.push({
      kind: e.body?.tool ?? 'tool',
      label: e.body?.label ?? '',
      ...(e.body?.title && { title: e.body.title }),
      ...(e.body?.detail && { detail: e.body.detail }),
      ...(e.body?.file && { file: e.body.file }),
      ...(e.body?.output ? { out: e.body.output, seq: e.seq } : o?.text ? { out: o.text, seq: o.seq } : {}),
    });
  }

  // Drop a trailing text event if it duplicates the final rendered answer (whitespace aside).
  if (opts.chatMode && !opts.running && opts.finalText) {
    const finals = (typeof opts.finalText === 'string' ? [opts.finalText] : opts.finalText).filter((x): x is string => !!x?.trim()).map(squash);
    const last = out.at(-1);
    if (last?.kind === 'text' && finals.includes(squash(last.label ?? ''))) out.pop();
  }
  if (!opts.chatMode && opts.notes) {
    out.push({ kind: 'text', label: opts.notes });
  }
  return out;
}

function findLastOpenToolSeq(events: readonly RawTurnEvent[], outputs: ReadonlyMap<string, unknown>): number {
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i];
    if (!e || kindOf(e) !== 'tool') continue;
    const done = Boolean(e.body?.output) || (e.body?.callId ? outputs.has(e.body.callId) : false);
    if (!done) return e.seq;
  }
  return -1;
}

function keepTool(vis: ToolCallVisibility, active: boolean): boolean {
  if (vis === 'showAll') return true;
  if (vis === 'hideSuccessful') return active;
  return false;
}

function filterFallback(activity: readonly TranscriptStepRow[], opts: ChatStepOpts): TranscriptStepRow[] {
  const out: TranscriptStepRow[] = [];
  for (const a of activity) {
    if (a.kind === 'error' || a.bad) {
      out.push(a);
    } else if (a.kind === 'thinking') {
      if (opts.showThinking && (!opts.chatMode || opts.toolCalls !== 'zen')) out.push(a);
    } else if (a.kind === 'text') {
      if (!opts.chatMode || opts.toolCalls !== 'zen') out.push(a);
    } else if (!opts.chatMode || opts.toolCalls === 'showAll') {
      out.push(a);
    }
  }
  if (!opts.chatMode && opts.notes) out.push({ kind: 'text', label: opts.notes });
  return out;
}
