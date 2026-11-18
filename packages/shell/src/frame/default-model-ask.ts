// What a harness's Default runs in a folder (runtime.defaultModel, owner's delegate 2026-11-18): the harness reads its own
// config there (a project that pins a model, else this computer's), never a run. Its own chunk, fetched on the first
// lookup (runner.ts useDefaultWords), apart from the first page's bundle.
import { defineCommand, type Client } from '@wisp/client';

/** runtime.defaultModel: the model's name, or null when the harness can't tell. */
export const defaultModelOf = defineCommand<{ peer: string; agent: string; cwd?: string }, string | null>({
  id: 'runtime.defaultModel',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent, ...(a.cwd && { cwd: a.cwd }) }),
  optimistic: 'none',
  pending: 'the default model',
  quiet: true,
});

export const askDefault = (client: Pick<Client, 'commands'>, a: { peer: string; agent: string; cwd?: string }): Promise<string | null> =>
  client.commands.run(defaultModelOf, a);
