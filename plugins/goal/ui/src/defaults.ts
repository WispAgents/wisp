// The default judges and vote (v1 S.config.defaults.goalJudges / goalVote): the `goal` resource on this wisp, read once
// per plugin client and kept in a signal both New task's Judges and Set goal… read; a change here shows at once and
// is saved to the server (its other fields, the round limit, as they were). A goal sent without its own judges is
// judged by these on the server, so this is the one place they live.

import { signal, type PeerId, type PluginClient, type Readable, type WritableSignal } from '@wisp/client';
import { goalDefaultsNow, judgeOf, saveGoalDefaults, type GoalResource } from './commands.ts';
import { VOTES, type Judge, type MemberRow, type Vote } from './types.ts';

export interface GoalDefaults {
  /** the judges as the server names them (a crew member's id, `harness/model/effort`), or as picked here */
  readonly judges: readonly (string | Judge)[];
  readonly vote: Vote;
}

const EMPTY: GoalDefaults = { judges: [], vote: 'majority' };
const held = new WeakMap<PluginClient, Map<string, WritableSignal<GoalDefaults>>>();
const slot = (client: PluginClient, peer: string): WritableSignal<GoalDefaults> | undefined => held.get(client)?.get(peer);

/** What the server's `goal` resource says, as defaults. */
export function defaultsOf(have: readonly GoalResource[]): GoalDefaults {
  const spec = have.find((r) => r.kind === 'goal' && r.name === 'default')?.spec;
  const judges = Array.isArray(spec?.['judges']) ? (spec['judges'] as unknown[]).filter((x): x is string => typeof x === 'string') : [];
  return { judges, vote: VOTES.includes(spec?.['vote'] as Vote) ? (spec?.['vote'] as Vote) : 'majority' };
}

/** The judges as judges, crew members told apart by id (read again as the crew arrives). */
export const judgesOf = (d: GoalDefaults, crew: readonly MemberRow[]): Judge[] => d.judges.map((j) => (typeof j === 'string' ? judgeOf(j, crew) : j));

/** The defaults, live: empty until the server answers (a change made meanwhile wins over the answer). */
export function goalDefaults(client: PluginClient, peer: string = client.home): Readable<GoalDefaults> {
  return slot(client, peer) ?? load(client, peer);
}

function load(client: PluginClient, peer: string = client.home): WritableSignal<GoalDefaults> {
  const made = signal<GoalDefaults>(EMPTY);
  const map = held.get(client) ?? new Map<string, WritableSignal<GoalDefaults>>();
  held.set(client, map);
  map.set(peer, made);
  void client
    .run(goalDefaultsNow, { peer: peer as PeerId })
    .then((have) => made() === EMPTY && made(defaultsOf(have)))
    .catch(() => {});
  return made;
}

/** Keep these as the defaults: shown at once, then saved. Saves go one after another (a quick second change can't land
 *  before the first), and a refused one puts the server's defaults back (the shell's toast says why). */
const queue = new WeakMap<PluginClient, Promise<unknown>>();
export function setGoalDefaults(client: PluginClient, judges: readonly Judge[], vote: Vote): void {
  const s = slot(client, client.home) ?? load(client);
  s({ judges, vote });
  const save = (): Promise<unknown> =>
    client
      .run(goalDefaultsNow, { peer: client.home })
      .then((have) => client.run(saveGoalDefaults, { peer: client.home, judges, vote, have }))
      .catch(() =>
        client
          .run(goalDefaultsNow, { peer: client.home })
          .then((have) => s(defaultsOf(have)))
          .catch(() => undefined),
      );
  queue.set(client, (queue.get(client) ?? Promise.resolve()).then(save));
}
