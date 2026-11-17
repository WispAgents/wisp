// Reading a task and the people who can judge it, for the goal's components: the task's entity key,
// its row, the crew and the harnesses. One place, so every component reads them the same way.

import type { EntityKey, PeerId, PluginClient } from '@wisp/client';
import { useDerived, useSignal } from '@wisp/ui';
import type { Accessor } from 'solid-js';
import { harnessOf, type HarnessTopicRow } from './rules.ts';
import { KIND, type HarnessRow, type MemberRow, type SessionRow } from './types.ts';

/**
 * A task's entity key. The shell hands extensions the row (whose id may already carry its origin)
 * and screens the key; a bare id is this client's own wisp's.
 */
export function keyOf(client: PluginClient, id: string): EntityKey {
  return (id.includes(':') ? id : `${client.home}:${id}`) as EntityKey;
}

/** The bare id the peer knows the task by. */
export const bareId = (key: string): string => key.slice(key.indexOf(':') + 1);

/** The task's row, live. */
export function useTask(client: PluginClient, key: () => EntityKey | undefined): Accessor<SessionRow | undefined> {
  const k = key();
  if (!k) return () => undefined;
  const row = useSignal(client.entities.row<SessionRow>(KIND.session, k));
  // the server's row keeps the list's fields (the goal among them) in `summary`: read them as the row's own
  return () => {
    const r = row() as (SessionRow & { summary?: Record<string, unknown> }) | undefined;
    return r?.summary && typeof r.summary === 'object' ? ({ ...r.summary, ...r } as SessionRow) : r;
  };
}

export function useCrew(client: PluginClient): Accessor<readonly MemberRow[]> {
  const list = useSignal(client.entities.list<MemberRow>(KIND.member));
  // the crew host keeps a member in its row's body ({id, name, updated_at, body})
  return () => (list() as readonly (MemberRow & { body?: Partial<MemberRow> })[]).map((r) => ({ ...r.body, ...r }) as MemberRow);
}

/** wisp.runtime's harnesses topic (ADR-0066) and its rows: every harness, each as its detect lands. */
export const HARNESSES_TOPIC = 'plugin:wisp.runtime:harnesses';
/** The harnesses on this wisp, in the picker's order. They're `runtime.harness` rows on wisp.runtime's harnesses
 *  topic, watched while the plugin lives (B47: a `harness` row nobody writes was read, so a judge's harness was
 *  never found: Add judge said none was installed and every harness judge was dropped). */
const watching = new WeakMap<PluginClient, Set<string>>();

/** `peer`: the machine the goal's task runs on (its judges run there), this wisp by default (B49). */
export function useHarnesses(client: PluginClient, peer: string = client.home): Accessor<readonly HarnessRow[]> {
  // once per plugin client and machine: the topic stays live while the plugin does
  const seen = watching.get(client) ?? new Set<string>();
  watching.set(client, seen);
  if (!seen.has(peer)) (seen.add(peer), client.watch(peer as PeerId, HARNESSES_TOPIC));
  return useDerived(() =>
    client.entities
      .keys(KIND.harness)()
      .filter((k) => k.startsWith(`${peer}:`))
      .map((k) => client.entities.row<HarnessTopicRow>(KIND.harness, k)() as HarnessTopicRow | undefined)
      .filter((r): r is HarnessTopicRow => r !== undefined && r.id.length > 0)
      .sort((a, b) => a.ord - b.ord)
      .map(harnessOf),
  );
}
