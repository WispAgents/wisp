// First paint from disk (plan §4.6.9): rows, cursors, pending commands, drafts and the last 200
// events of recent sessions. Render first, then hello{cursor} per peer.
//
// Rows and the link state that matches them are written in one batch, captured at one instant:
// a cursor newer than its rows would skip changes, one older would replay appends twice.
// Event lists are cut to their tail; the transcript pages further back with `window`.

import type { ChangedRow, EntityWriter, StoredRow } from './entities.ts';
import { ownerOf } from './ids.ts';
import type { LinkSaved } from './peer.ts';
import type { Persist, PersistWrite } from './persist.ts';
import type { Scope } from './scope.ts';
import type { EntityKey, PeerId, Warn } from './types.ts';

export interface SavedPeer {
  readonly link?: LinkSaved;
  readonly rows: readonly StoredRow[];
}

const LINK_NS = 'link';
const rowsNs = (peer: string): string => `rows:${peer}`;
const rowId = (kind: string, key: EntityKey): string => `${kind}\u0000${key}`;

export async function loadFirstPaint(persist: Persist): Promise<Map<PeerId, SavedPeer>> {
  const out = new Map<PeerId, SavedPeer>();
  const links = new Map((await persist.entries(LINK_NS)) as [PeerId, LinkSaved][]);
  const nss = await persist.namespaces('rows:');
  const peers = new Set<PeerId>([...links.keys(), ...nss.map((n) => n.slice('rows:'.length) as PeerId)]);
  for (const peer of peers) {
    const rows = (await persist.entries(rowsNs(peer))).map(([, v]) => v as StoredRow);
    const link = links.get(peer);
    out.set(peer, link ? { link, rows } : { rows });
  }
  return out;
}

/** Puts saved rows into the store's base, at the cursor they were saved with. */
export function restoreFirstPaint(writer: EntityWriter, saved: ReadonlyMap<PeerId, SavedPeer>): void {
  // the topics the saved link held whole as of its cursor (synced, or held as of a seq): loaded until a reset says not
  for (const [peer, s] of saved) writer.restore(peer, s.link?.cursor ?? 0, s.rows, [...(s.link?.synced ?? []), ...(s.link?.held ?? []).map(([t]) => t)]);
}

export interface PersistEntitiesOptions {
  writer: EntityWriter;
  persist: Persist;
  scope: Scope;
  /** the link state to save with the rows */
  linkOf(peer: PeerId): LinkSaved | undefined;
  debounceMs?: number;
  /** the entity kind of transcript events (rows are lists of events per session) */
  eventKind?: string;
  eventsPerSession?: number;
  recentSessions?: number;
  warn?: Warn;
}

export interface EntityPersister {
  flush(): Promise<void>;
}

/** rows a save writes per transaction */
const SAVE_CHUNK = 500;

export function persistEntities(o: PersistEntitiesOptions): EntityPersister {
  const wait = o.debounceMs ?? 250;
  const eventKind = o.eventKind ?? 'event';
  const keep = o.eventsPerSession ?? 200;
  const recent = o.recentSessions ?? 30;
  const dirty = new Map<string, ChangedRow>();
  const eventOrder: EntityKey[] = []; // event rows by last change, oldest first
  let cancel: (() => void) | undefined;
  let chain: Promise<void> = Promise.resolve();

  const touchEvents = (key: EntityKey): EntityKey[] => {
    const i = eventOrder.indexOf(key);
    if (i >= 0) eventOrder.splice(i, 1);
    eventOrder.push(key);
    return eventOrder.length > recent ? eventOrder.splice(0, eventOrder.length - recent) : [];
  };

  function capture(): PersistWrite[] {
    const writes: PersistWrite[] = [];
    const peers = new Set<PeerId>();
    for (const { kind, key } of dirty.values()) {
      const peer = ownerOf(key);
      peers.add(peer);
      let row = o.writer.stored(kind, key);
      if (row && kind === eventKind && Array.isArray(row.v)) {
        row = { ...row, v: row.v.slice(-keep) };
        for (const old of touchEvents(key)) writes.push({ ns: rowsNs(ownerOf(old)), key: rowId(kind, old), value: undefined });
      }
      writes.push({ ns: rowsNs(peer), key: rowId(kind, key), value: row });
    }
    for (const peer of peers) writes.push({ ns: LINK_NS, key: peer, value: o.linkOf(peer) });
    dirty.clear();
    return writes;
  }

  function flush(): Promise<void> {
    cancel?.();
    cancel = undefined;
    const run = async (): Promise<void> => {
      if (dirty.size === 0) return;
      // in chunks, each its own transaction (and task): 5 000 rows in one were a long task as the first screen settled
      // (perf r1 #4). The links come last in `capture()`, so a save cut short never records a cursor ahead of its rows.
      // Serialized on `chain` so overlapping flushes cannot overwrite a newer link cursor with an older one.
      const all = capture();
      for (let i = 0; i < all.length; i += SAVE_CHUNK) await o.persist.write(all.slice(i, i + SAVE_CHUNK));
    };
    const next = chain.then(run, run);
    chain = next.catch(() => {});
    return next;
  }

  const stop = o.writer.onChange((rows) => {
    for (const r of rows) dirty.set(rowId(r.kind, r.key), r);
    cancel ??= o.scope.timeout(wait, () => {
      cancel = undefined;
      flush().catch((err: unknown) => o.warn?.('rows not saved', String(err)));
    });
  });
  o.scope.add(stop);
  return { flush };
}
