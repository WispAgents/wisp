// What this wisp is called, for a component (here.ts's words, ADR-0148): apart from here.ts so plain modules and their
// tests don't compile the kit.
import { useSignal, type IconName } from '@wisp/ui';
import { useShell } from '../context.tsx';

/** The same, for a component: read through `useSignal`, so the hub's name shows once its welcome comes (a read of the
 *  client's store in JSX isn't followed by Solid). */
export interface Here {
  readonly hub: () => boolean;
  readonly name: () => string;
  readonly icon: () => IconName;
}

export function useHere(): Here {
  const shell = useShell();
  const home = shell.client.home();
  const info = useSignal(shell.client.peers.info(home));
  const hub = (): boolean => info()?.kind === 'hub';
  return {
    hub,
    name: () => (hub() ? info()?.name || shell.client.peers.name(home) || shell.t('shell.card.thisComputer') : shell.t('shell.card.thisComputer')),
    icon: () => (hub() ? 'server' : 'laptop'),
  };
}
