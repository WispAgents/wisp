/** The view `/sandbox` opens at once: the launch's page while its sandbox comes up, then its task. */
export const LAUNCHING = 'sandbox.launching';

export interface LaunchingParams {
  /** the launch's id (its `sandbox.launch` row) */
  readonly id: string;
  /** what was typed: the title before the row is there */
  readonly prompt?: string;
}

/** The view's params from navigation; anything else isn't this view's (thrown: the shell lands on Home). */
export function parseLaunching(p: unknown): LaunchingParams {
  const o = (p ?? {}) as Record<string, unknown>;
  if (typeof o['id'] !== 'string') throw new Error('sandbox.launching: { id, prompt? }');
  return { id: o['id'], ...(typeof o['prompt'] === 'string' && { prompt: o['prompt'] }) };
}
