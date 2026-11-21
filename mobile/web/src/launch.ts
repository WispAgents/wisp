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
//
// Tasks waits for a computer (tasksReady): its destination is dimmed until you picked the shell's computer (now, or on
// an earlier run) and it has been online. Launching one that isn't online yet keeps you on Computers (its row says
// Connecting) and opens its tasks the moment it connects, unless you went elsewhere meanwhile.

import { computed, signal, subscribe, untracked, type Client, type PeerId, type Readable, type Scope } from '@wisp/client';
import { COMPUTERS_VIEW } from '@wisp/shell/pure';
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
  /** the computer you launched in this run (none before the first press) */
  readonly picked: Readable<PeerId | undefined>;
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
  const picked = signal<PeerId | undefined>(undefined);
  let waiting: (() => void) | undefined; // the launch still waiting for its computer to connect
  const stopWaiting = (): void => {
    waiting?.();
    waiting = undefined;
  };
  o.scope.add(stopWaiting);
  const onComputers = (): boolean => {
    const main = untracked(o.client.nav.nav).main;
    return main.kind === 'view' && String(main.id) === COMPUTERS_VIEW;
  };
  return {
    picked,
    launch(c) {
      stopWaiting();
      if (!untracked(o.client.peers.list()).includes(c.peer)) return false;
      o.storage?.setItem(LAUNCHED, c.id);
      picked(c.peer);
      o.client.setHome(c.peer); // the same one: nothing changes, nothing runs again
      const status = o.client.peers.status(c.peer);
      if (untracked(status).kind === 'online') {
        o.client.nav.navigate({ t: 'home' }); // its tasks (the Tasks destination), in the press's frame whatever follows
        return true;
      }
      if (untracked(status).kind === 'unreachable') o.client.peers.reconnect(c.peer);
      // not connected yet: Tasks stays dimmed; its tasks open when it is, if you're still on Computers
      let done = false;
      const stop = subscribe(status, (s) => {
        if (done || s.kind !== 'online') return;
        done = true;
        queueMicrotask(stopWaiting);
        if (onComputers()) o.client.nav.navigate({ t: 'home' });
      });
      waiting = stop;
      return true;
    },
  };
}

export interface TasksReadyOptions {
  client: Pick<Client, 'home' | 'peers'>;
  launcher: Pick<Launcher, 'picked'>;
  storage?: StorageLike | undefined;
  /** the saved computers (the bridge's list): which peer the computer launched on an earlier run is now */
  list: () => Promise<readonly Computer[]>;
  /** the shell's: the watch goes with it */
  scope: Scope;
}

/** Whether the shell's Tasks can open: you picked its computer (now, or on an earlier run) and it has been online
 *  since this shell started. The phone (no computer yet) never is. One per shell: the home is read once. */
export function tasksReady(o: TasksReadyOptions): Readable<boolean> {
  const home = untracked(o.client.home);
  const status = o.client.peers.status(home);
  const seen = signal(untracked(status).kind === 'online');
  o.scope.add(subscribe(status, (s) => s.kind === 'online' && seen(true)));
  const earlier = signal(false);
  const last = o.storage?.getItem(LAUNCHED);
  if (last) {
    o.list().then(
      (list) => {
        if (o.scope.alive) earlier(peerMap(list).get(last) === home);
      },
      () => {}, // the bridge is gone with the page
    );
  }
  return computed(() => (o.launcher.picked() === home || earlier()) && seen());
}
