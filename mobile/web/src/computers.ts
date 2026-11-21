// The Computers screen's rows (ADR-0065 step 8, design/mobile.md §A.2 Computers), from the saved computers and each
// one's peer: its name, how it's reached, its state as a word (*Online*, *Connecting*, *Offline*, *Asleep*, never a
// disabled button), and which one is launched (the shell's home: new tasks, its settings). Pure.

import type { PeerId, PeerStatus } from '@wisp/client';
import type { Computer } from './bridge.ts';

export type ComputerState = 'online' | 'connecting' | 'offline' | 'asleep' | 'update';

export interface ComputerRow {
  readonly id: string;
  readonly peer: PeerId;
  readonly name: string;
  /** how the phone reaches it: an i18n key's last part (`phone.via.<via>`) */
  readonly via: 'ssh' | 'cloud' | 'grid';
  readonly state: ComputerState;
  /** the launched computer: the shell runs on it */
  readonly current: boolean;
}

export function stateOf(s: PeerStatus | undefined): ComputerState {
  switch (s?.kind) {
    case 'online':
      return 'online';
    case 'connecting':
    case 'waking':
      return 'connecting';
    case 'asleep':
      return 'asleep';
    case 'incompatible':
      return 'update';
    default:
      return 'offline';
  }
}

/** One row per saved computer, in the saved order. `peers` maps a saved id to its peer (peerMap). */
export function computerRows(list: readonly Computer[], peers: ReadonlyMap<string, PeerId>, status: (peer: PeerId) => PeerStatus | undefined, home: PeerId): ComputerRow[] {
  const out: ComputerRow[] = [];
  for (const c of list) {
    const peer = peers.get(c.id);
    if (!peer) continue;
    const via = c.kind === 'ssh' ? 'ssh' : c.computer ? 'grid' : 'cloud';
    out.push({ id: c.id, peer, name: c.name, via, state: stateOf(status(peer)), current: peer === home });
  }
  return out;
}
