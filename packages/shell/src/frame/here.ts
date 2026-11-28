// What the wisp this app runs on is called (ADR-0148): "This computer" on a person's own, and on a team's hub its name
// ("Acme hub"), which its welcome says (`PeerInfo.kind: 'hub'`): a hub is nobody's computer, and its people read
// "This computer" as their own laptop. Every place that names "this one" asks here.
import type { IconName } from '@wisp/ui';

export interface HereClient {
  home(): string;
  readonly peers: {
    info(peer: never): () => { readonly kind?: string; readonly name: string } | undefined;
    name(peer: never): string;
  };
}

/** Whether this wisp is a team's hub. */
export const onHub = (c: HereClient): boolean => c.peers.info(c.home() as never)()?.kind === 'hub';

/** "This computer", or the hub's name. */
export function hereName(c: HereClient, t: (k: string) => string): string {
  if (!onHub(c)) return t('shell.card.thisComputer');
  const info = c.peers.info(c.home() as never)();
  return info?.name || c.peers.name(c.home() as never) || t('shell.card.thisComputer');
}

/** A laptop, or a server for a hub. */
export const hereIcon = (c: HereClient): IconName => (onHub(c) ? 'server' : 'laptop');

