// Continue on… (ADR-0125), DOM-free so tests read it: the dialog's params, what its preview says each choice carries
// (summary only, the default, or the full transcript) with its token estimate, the follow-up it sends, and the
// Timeline's divider from a turn's switch mark.
import type { EntityKey } from '@wisp/client';
import '../i18n/lazy.ts';
import { viewParams, type ContinueParams } from './view.ts';

export { CONTINUE_VIEW, type ContinueParams } from './view.ts';
export { openContinue } from './open.ts';

/** The dialog's params as it reads them: the view's check (a task's key), then each of the rest a word or absent. */
export function parseContinueParams(v: unknown): ContinueParams {
  const o = viewParams(v) as unknown as Record<string, unknown>;
  const str = (k: string): string | undefined => (typeof o[k] === 'string' && o[k] !== '' ? (o[k] as string) : undefined);
  const agent = str('agent');
  const model = str('model');
  const text = str('text');
  return { key: o['key'] as EntityKey, ...(agent && { agent }), ...(model && { model }), ...(text && { text }) };
}

/** what a hand-off carries: the latest turn and the rules, or every turn the harness missed */
export type HandoffChoiceId = 'summary' | 'full';
export const CHOICES: readonly HandoffChoiceId[] = ['summary', 'full'];
/** what carrying on says when nothing is typed */
export const CARRY_ON = 'Carry on with this task from where it is.';

/** `runtime.handoffPreview`'s answer (runtime-api HandoffPreviewView). */
export interface HandoffPreview {
  readonly summary: string;
  readonly files: readonly string[];
  readonly missed: number;
  readonly rules: number;
  readonly brief: { readonly turns: number; readonly tokens: number };
  readonly full: { readonly turns: number; readonly tokens: number };
}

type Say = (key: string, vars?: Readonly<Record<string, string | number>>) => string;

/** "~1.2k tokens", "~340 tokens" */
export function tokenWords(n: number, t: Say): string {
  const v = n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1).replace(/\.0$/, '')}k` : String(Math.max(0, Math.round(n)));
  return t('shell-task.continue.tokens', { n: v });
}

export interface PreviewRow {
  readonly id: 'summary' | 'files' | 'turns' | 'rules';
  readonly label: string;
  readonly text: string;
}

/** What the sheet lists for a choice: the latest summary, the files, the turns written out (all of them only for the
 *  full transcript), the user's rules; and the choice's size. Nothing missed: it has seen everything, nothing goes. */
export function previewModel(p: HandoffPreview | undefined, choice: HandoffChoiceId, t: Say): { rows: PreviewRow[]; tokens: string; options: { id: HandoffChoiceId; label: string; hint: string }[]; nothing: boolean } {
  const options = CHOICES.map((id) => ({
    id,
    label: t(`shell-task.continue.choice.${id}`),
    hint: p ? tokenWords(id === 'full' ? p.full.tokens : p.brief.tokens, t) : '',
  }));
  if (!p) return { rows: [], tokens: '', options, nothing: false };
  if (p.missed === 0) return { rows: [], tokens: tokenWords(0, t), options, nothing: true };
  const c = choice === 'full' ? p.full : p.brief;
  const rows: PreviewRow[] = [];
  if (p.summary) rows.push({ id: 'summary', label: t('shell-task.continue.summary'), text: p.summary });
  if (p.files.length > 0) rows.push({ id: 'files', label: t('shell-task.continue.files'), text: p.files.slice(0, 6).join(', ') + (p.files.length > 6 ? ` +${p.files.length - 6}` : '') });
  rows.push({
    id: 'turns',
    label: t('shell-task.continue.turns'),
    text: choice === 'full' ? t('shell-task.continue.turnsFull', { count: c.turns }) : t('shell-task.continue.turnsBrief', { count: p.missed }),
  });
  if (p.rules > 0) rows.push({ id: 'rules', label: t('shell-task.continue.rules'), text: t('shell-task.continue.rulesN', { count: p.rules }) });
  return { rows, tokens: tokenWords(c.tokens, t), options, nothing: false };
}

/** The follow-up Continue sends: the words (or carry on), the harness and model, what it carries. One call: the
 *  runtime switches the task's harness with the turn and marks it (ADR-0125). */
export function continueArgs(key: EntityKey, to: { readonly agent: string; readonly model: string }, choice: HandoffChoiceId, text: string): {
  key: EntityKey;
  text: string;
  run: { agent: string; model?: string };
  handoff: HandoffChoiceId;
} {
  return { key, text: text.trim() || CARRY_ON, run: { agent: to.agent, ...(to.model && { model: to.model }) }, handoff: choice };
}

/** A turn's switch mark (`ext["wisp.runtime"].switch`, written by the runtime). */
export interface SwitchMark {
  readonly from: { readonly agent: string; readonly model: string };
  readonly to: { readonly agent: string; readonly model: string };
  readonly context: HandoffChoiceId;
}

export function switchOf(turn: unknown): SwitchMark | undefined {
  const b = (turn ?? {}) as { body?: unknown; ext?: unknown };
  const ext = ((b.body as { ext?: unknown } | undefined)?.ext ?? b.ext) as Record<string, unknown> | undefined;
  const sw = (ext?.['wisp.runtime'] as { switch?: unknown } | undefined)?.switch as Partial<SwitchMark> | undefined;
  if (!sw?.from?.agent || !sw.to?.agent) return undefined;
  return { from: { agent: sw.from.agent, model: sw.from.model ?? '' }, to: { agent: sw.to.agent, model: sw.to.model ?? '' }, context: sw.context === 'summary' ? 'summary' : 'full' };
}

/** "Switched from Claude Code (Opus) to Codex (GPT-5) · context: summary"; a model it doesn't name is left out. */
export function dividerWords(m: SwitchMark, nameOf: (agent: string) => string, modelOf: (agent: string, model: string) => string, t: Say): string {
  const side = (s: SwitchMark['from']): string => {
    const name = nameOf(s.agent);
    const model = s.model ? modelOf(s.agent, s.model) : '';
    return model ? `${name} (${model})` : name;
  };
  return t('shell-task.continue.divider', { from: side(m.from), to: side(m.to), context: t(`shell-task.continue.context.${m.context}`) });
}
