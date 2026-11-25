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

export function parseContinueParams(v: unknown): ContinueParams {
  const o = (v ?? {}) as Record<string, unknown>;
  if (typeof o['key'] !== 'string' || !o['key'].includes(':')) throw new Error('Continue on… needs the task’s key');
  const str = (k: string): string | undefined => (typeof o[k] === 'string' && o[k] !== '' ? (o[k] as string) : undefined);
  const agent = str('agent');
  const model = str('model');
  const text = str('text');
  return { key: o['key'] as EntityKey, ...(agent && { agent }), ...(model && { model }), ...(text && { text }) };
}
