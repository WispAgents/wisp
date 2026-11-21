// This wisp's detect rows (ADR-0066), for Settings → Agents: one per harness, checking until its CLI answers, then
// settled in place. Through useDerived (round 324): a createMemo over the keys read each row once, so a harness whose
// detect landed after its key showed stayed "Checking…" until the pane was opened again.
import { ownerOf, type EntityKey, type EntityReader } from '@wisp/client';
import { useDerived } from '@wisp/ui/pure';
import type { Accessor } from 'solid-js';
import { DETECT, type DetectRow } from './types.ts';

export function useDetected(entities: Pick<EntityReader, 'keys' | 'row'>, peer: string): Accessor<DetectRow[]> {
  return useDerived(() =>
    entities
      .keys(DETECT)()
      .filter((k) => ownerOf(k) === peer)
      .map((k) => entities.row<DetectRow>(DETECT, k as EntityKey)() as DetectRow | undefined)
      .filter((r): r is DetectRow => r !== undefined),
  );
}
