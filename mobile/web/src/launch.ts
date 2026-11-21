// Launching a computer (ADR-0065 step 8): the phone keeps every saved computer connected at once, each its own peer
// with its own tunnel or grid link and its own keys (the native side, round 226). Launching one makes it the shell's
// home, as v1's app opened that computer's wisp: new tasks start there, its settings and Machines are the ones shown,
// a bare task link is its. No peer is added, dropped or reconnected by a switch (only an unreachable one is asked to
// try now instead of after its backoff), so going back to another computer finds it as it was.
//
// A launched computer re-keyed by its first welcome, or forgotten, moves the home in followComputers (peers.ts), which
// adds and removes the peers; the launcher runs the shell again whenever the home changes.
//
// The launched computer is kept on the phone (`wisp.phone.launched`, a saved id: no token, nothing secret), so the
// next start runs the shell on it again, while the app still opens on Computers (ADR-0065 open question 3).

import { subscribe, untracked, type Client, type PeerId, type Scope } from '@wisp/client';
import type { Computer } from './bridge.ts';
import type { StorageLike } from './migrate.ts';
import { peerMap, PHONE } from './peers.ts';

export const LAUNCHED = 'wisp.phone.launched';

/** The home to start with: the computer launched last, else the first saved one, else the phone (no computer yet). */
export function startHome(list: readonly Computer[], storage: StorageLike | undefined): PeerId {
  const peers = peerMap(list);
  const last = storage?.getItem(LAUNCHED);
  return (last ? peers.get(last) : undefined) ?? peers.values().next().value ?? PHONE;
}

export interface Launcher {
  /** runs the shell on that computer and shows its tasks; false (its tasks shown, the shell where it was) while its peer
   *  isn't there yet (a re-key landing) */
  launch(c: { readonly id: string; readonly peer: PeerId }): boolean;
}

export interface LauncherOptions {
  client: Pick<Client, 'home' | 'setHome' | 'peers' | 'nav'>;
  storage?: StorageLike | undefined;
  /** the home changed (a launch, or followComputers re-keying or forgetting the launched computer): the app runs its
   *  shell again on the new one (what's mounted read the old home) */
  onSwitch?: (peer: PeerId) => void;
  /** owns the watch on the home */
  scope: Scope;
}

export function createLauncher(o: LauncherOptions): Launcher {
  o.scope.add(subscribe(o.client.home, (peer) => o.onSwitch?.(peer)));
  return {
    launch(c) {
      o.client.nav.navigate({ t: 'home' }); // its tasks (the Tasks destination), in the press's frame whatever follows
      if (!untracked(o.client.peers.list()).includes(c.peer)) return false;
      o.storage?.setItem(LAUNCHED, c.id);
      o.client.setHome(c.peer); // the same one: nothing changes, nothing runs again
      if (untracked(o.client.peers.status(c.peer)).kind === 'unreachable') o.client.peers.reconnect(c.peer);
      return true;
    },
  };
}
