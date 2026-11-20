// What a picked formula or task file runs (audit H5): on the first screen (HomeView's runner pill reads it), so it
// imports nothing; formula-pick.ts, which reads the file, loads only when Formula or task file… is pressed.
import type { FormulaPick } from './formula-pick.ts';

/** What a picked file runs (audit H5: the bar said Claude Code and Default while the file's codex ran): its task's agent
 *  as the formula declares it (a name, its harness, model and effort), or the harness and model the task names. */
export interface FileRunner {
  /** the agent the task names (`critic`), or its harness when the formula declares no agent of that name */
  readonly name: string;
  readonly harness: string;
  readonly model: string;
  readonly effort: string;
}

const word = (v: unknown): string => (typeof v === 'string' ? v : '');

export function fileRunner(p: FormulaPick | undefined): FileRunner | undefined {
  if (!p) return undefined;
  const agent = word(p.args.agent);
  const model = word(p.args.model);
  if (!agent && !model) return undefined;
  const list = (p.args.formula as { readonly agents?: unknown } | undefined)?.agents;
  const decl = Array.isArray(list) ? (list as readonly Record<string, unknown>[]).find((a) => a['name'] === agent) : undefined;
  return { name: agent, harness: word(decl?.['harness']) || agent, model: model || word(decl?.['model']), effort: word(p.args.effort) || word(decl?.['effort']) };
}
