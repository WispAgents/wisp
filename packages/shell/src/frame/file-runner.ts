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

/** The bar's own picks (its model, thinking and account), as New task sends them with the prompt. */
export interface BarPicks {
  readonly agent: string;
  readonly model?: string;
  readonly effort?: string;
  readonly account?: string;
}

/** What of the bar's picks goes with a task (audit I4: with a task file picked, `runtime.create` carried the file's
 *  `critic` and the bar's `"model":"opus"`, and codex ran `-m opus`). A picked file that names what runs (its agent, or a
 *  model) brings its own model and thinking: none of the bar's go, so what the pill says is what runs. Auto sends none. */
export function barArgs(file: FileRunner | undefined, bar: BarPicks): { model?: string; effort?: string; account?: string } {
  if (file || bar.agent === 'auto') return {};
  return { ...(bar.model && { model: bar.model }), ...(bar.effort && { effort: bar.effort }), ...(bar.account && { account: bar.account }) };
}
