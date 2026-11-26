// Copy turn as Markdown (R4-6): one turn as text to paste in an issue, a PR or a chat. The prompt, each step (its
// label, the command it ran, what it printed in a fence) and the answer as the agent wrote it (its markdown source,
// not the page's words). Its own module, loaded on the click (ADR-0011). It copies what this client holds: a step's
// output as the transcript shows it, a .env file's masked text unless that step was revealed (stepOutput).
// Where the server cut a long output, the cut text is what's copied; runtime.fullOutput is where the whole of it
// would come from once the server has it (asked per step here, before serializing).
import { stepIdOf } from './step-open.ts';
import { stepOutput, type TranscriptStepRow } from './TurnTranscript.tsx';
import type { FindEnv } from './task-find.ts';

export interface TurnMarkdownIn {
  readonly prompt: string;
  readonly steps: readonly TranscriptStepRow[];
  /** the answer's markdown source: what the agent wrote, or its presentation's markdown blocks */
  readonly answer?: string;
  readonly env?: FindEnv;
  /** the task's folder taken out of paths, as the transcript does */
  readonly here?: (s: string) => string;
  readonly words: { readonly prompt: string; readonly steps: string; readonly answer: string };
}

/** A fence that `text` can't close: one backtick longer than its longest run, three at least. */
export function fenceFor(text: string): string {
  const longest = Math.max(0, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  return '`'.repeat(Math.max(3, longest + 1));
}

/** An inline code span `text` can't close: a backtick run longer than any inside it (Markdown honours no backslash
 *  escape in a code span), padded with a space when the text starts or ends with a backtick. */
export function codeSpan(text: string): string {
  const longest = Math.max(0, ...[...text.matchAll(/`+/g)].map((m) => m[0].length));
  const tick = '`'.repeat(longest + 1);
  const pad = text.startsWith('`') || text.endsWith('`') ? ' ' : '';
  return `${tick}${pad}${text}${pad}${tick}`;
}

const fenced = (text: string): string => {
  const f = fenceFor(text);
  return `${f}\n${text.replace(/\n$/, '')}\n${f}`;
};

function stepMarkdown(a: TranscriptStepRow, index: number, env: FindEnv | undefined, here: (s: string) => string): string {
  if (a.kind === 'text') return here(a.label ?? '');
  const id = stepIdOf(a, index);
  const head = `- **${here(a.title ?? a.label ?? a.kind).replace(/\s+/g, ' ').trim()}**`;
  const cmd = a.kind === 'run' && a.detail && !(a.label ?? '').includes(a.detail.slice(0, 24)) ? `\n\n  ${codeSpan(here(a.detail).replace(/\s*\n\s*/g, ' '))}` : '';
  const shown = a.kind === 'thinking' ? a.out : stepOutput(a, env?.on() ?? true, env?.shown(id) ?? false, env?.raw(id))?.text;
  const out = shown && shown !== a.label ? `\n\n${fenced(here(shown))}` : '';
  return head + cmd + out;
}

/** The turn as Markdown. */
export function turnMarkdown(x: TurnMarkdownIn): string {
  const here = x.here ?? ((s: string) => s);
  const parts = [`## ${x.words.prompt}`, x.prompt.trim()];
  if (x.steps.length) parts.push(`## ${x.words.steps}`, x.steps.map((a, i) => stepMarkdown(a, i, x.env, here)).join('\n\n'));
  if (x.answer?.trim()) parts.push(`## ${x.words.answer}`, x.answer.trim());
  return `${parts.join('\n\n')}\n`;
}
