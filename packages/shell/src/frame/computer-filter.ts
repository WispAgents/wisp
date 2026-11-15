// The Tasks list's computer filter in the phone apps (ADR-0065 step 13, design/mobile.md §A.1, §A.2): the list holds
// the rows of every saved computer (the client's peers, merged), and the top app bar's title narrows it to one: *All*
// or a computer's name. The pick is the device setting `shell.computerFilter` ('all' or a peer id), as the project
// filter is `shell.projectFilter`; a computer that's gone (forgotten, re-keyed) reads as All. A row is a computer's
// when its key is `<peer>:…` (@wisp/client entityKey). Pure.

export const ALL_COMPUTERS = 'all';

export interface ComputerOption {
  /** 'all' or a peer id */
  readonly id: string;
  readonly label: string;
  readonly on: boolean;
}

/** The pick, or All when it names a computer the phone no longer has. */
export function knownComputer(filter: string | undefined, peers: readonly string[]): string {
  return filter && filter !== ALL_COMPUTERS && peers.includes(filter) ? filter : ALL_COMPUTERS;
}

/** The bar's title: *All*, or the picked computer's name. */
export function computerTitle(filter: string | undefined, peers: readonly string[], name: (peer: string) => string, allLabel: string): string {
  const on = knownComputer(filter, peers);
  return on === ALL_COMPUTERS ? allLabel : name(on) || on;
}

/** What the title's sheet offers: All first, then each computer in the client's order, the pick marked. */
export function computerOptions(filter: string | undefined, peers: readonly string[], name: (peer: string) => string, allLabel: string): ComputerOption[] {
  const on = knownComputer(filter, peers);
  return [{ id: ALL_COMPUTERS, label: allLabel, on: on === ALL_COMPUTERS }, ...peers.map((p) => ({ id: p, label: name(p) || p, on: on === p }))];
}

/** Whether a row (by its entity key) shows under the pick. */
export function inComputer(filter: string, key: string): boolean {
  return filter === ALL_COMPUTERS || key.startsWith(`${filter}:`);
}
