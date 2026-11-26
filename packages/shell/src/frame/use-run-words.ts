// A run in words for any screen, a plugin's too (QA r15 #3): "Claude Code · Opus · High", never "claude · opus · high".
// The harness's name as its plugin registered it, the model as the picker names it (the harnesses topic's label, else
// a Claude id in words), the thinking level in words; a run on its Default names what Default runs there (the same
// answer the chip gives, default-models.ts), else the harness alone.
import { useSignal } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import { defaultModelName, defaultModels } from './default-models.ts';
import { modelWords, runLabel } from './run-words.ts';
import '../i18n/home.ts';

/** What a run was given: its harness id, model id, thinking level; where it runs (its Default is named there). */
export interface RunIds {
  readonly agent: string;
  readonly model?: string | undefined;
  readonly effort?: string | undefined;
  /** its computer (default: this client's own) and folder: what its Default runs is asked there */
  readonly peer?: string | undefined;
  readonly cwd?: string | undefined;
}

export function useRunWords(shell: Pick<ShellContext, 'client' | 'harnesses' | 't'>): (r: RunIds) => string {
  const harnesses = useSignal(shell.harnesses());
  const home = useSignal(shell.client.home);
  const store = defaultModels(shell.client);
  return (r) => {
    if (!r.agent) return '';
    const name = harnesses().find((h) => h.id === r.agent)?.name ?? (r.agent === 'auto' ? shell.t('shell.home.auto') : r.agent);
    const model = r.model ? (store.label(r.agent, r.model) ?? modelWords(r.model)) : r.agent === 'auto' ? undefined : defaultModelName(shell.client, r.peer ?? home(), r.agent, r.cwd);
    return runLabel({ name, model, effort: r.effort });
  };
}
