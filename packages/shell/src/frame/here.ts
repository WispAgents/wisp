// What the wisp this app runs on is called (ADR-0148): "This computer" on a person's own; on a team's hub its name
// ("Acme hub"), as its welcome says (`PeerInfo.kind: 'hub'`). Components read it through `useHere` (use-here.ts).

/** The hub's name when this wisp is a team's hub, else none. */
export const hubName = (c: { home(): string; readonly peers: { info(p: never): () => { readonly kind?: string; readonly name: string } | undefined } }): string | undefined => {
  const i = c.peers.info(c.home() as never)();
  return i?.kind === 'hub' ? i.name : undefined;
};
