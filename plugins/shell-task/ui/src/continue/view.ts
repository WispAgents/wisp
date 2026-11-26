// Continue on…'s view (ADR-0125): its kind and its params, all the plugin's entry needs to register the dialog. How it
// opens (open.ts), the rest of the model (model.ts) and its strings (en.lazy.json) load later, not with the first
// screen (ADR-0011).
import type { EntityKey } from '@wisp/client';

export const CONTINUE_VIEW = 'shell-task.continue';

export interface ContinueParams {
  readonly key: EntityKey;
  /** a harness (and model) already picked: a refused or limited turn's offer */
  readonly agent?: string;
  readonly model?: string;
  /** the follow-up's words (a refused prompt again, `/continue …` typed in the reply box) */
  readonly text?: string;
}

/** The view's own check, all the first screen carries (ADR-0011): a task's key. The dialog reads the rest
 *  (`parseContinueParams`, model.ts). */
export function viewParams(v: unknown): ContinueParams {
  if (!String((v as { key?: unknown } | null)?.key ?? '').includes(':')) throw new Error('Continue on… needs the task’s key');
  return v as ContinueParams;
}
