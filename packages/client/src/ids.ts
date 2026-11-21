// IDs: UUIDv7 mids (time-ordered idempotency keys) and origin-prefixed entity keys (plan §7.2.1).

import type { EntityKey, PeerId } from './types.ts';

/** A UUIDv7 (RFC 9562): 48 bits of ms time, version, 74 random bits. */
export function uuidv7(now: number = Date.now()): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  let t = now;
  for (let i = 5; i >= 0; i--) {
    b[i] = t % 256;
    t = Math.floor(t / 256);
  }
  b[6] = ((b[6] ?? 0) & 0x0f) | 0x70;
  b[8] = ((b[8] ?? 0) & 0x3f) | 0x80;
  const h = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

/**
 * A new row's id in the shape the peer makes its own (`Uuid::simple`: a UUIDv7's 32 hex digits, no dashes). A client
 * that shows a task before the peer answers names it with this and sends it (`runtime.create`'s `id`), so the row and
 * page it opened are the ones the peer writes (ADR-0017); the peer refuses any other shape.
 */
export function newRowId(now: number = Date.now()): string {
  return uuidv7(now).replaceAll('-', '');
}

export function isPeerId(s: string): s is PeerId {
  return /^[A-Za-z0-9._-]+$/.test(s);
}

export function peerId(s: string): PeerId {
  if (!isPeerId(s)) throw new Error(`bad peer id: ${s}`);
  return s;
}

/**
 * The key a row from `peer` is stored under. New IDs already carry their origin (`laptop:…`);
 * legacy ones, imported unchanged, get it added. Nothing on the wire is rewritten.
 */
export function entityKey(peer: PeerId, id: string): EntityKey {
  return (id.startsWith(`${peer}:`) ? id : `${peer}:${id}`) as EntityKey;
}

// one bare id → key index per keys list: `EntityReader.keys` gives a new array on each change, so every caller holding
// the same list (⌘K's 4,500 tasks, each list row, the nav) shares one Map instead of scanning it per id (perf-ui-r2 #1)
const byBareId = new WeakMap<readonly string[], ReadonlyMap<string, string>>();

/** The key in `keys` whose id (what follows its `<peer>:`) is `id`, the first in order when two peers have it; O(1)
 *  after the list's first lookup. */
export function keyOfId<K extends string>(keys: readonly K[], id: string): K | undefined {
  let m = byBareId.get(keys);
  if (!m) {
    const built = new Map<string, string>();
    for (const k of keys) {
      const bare = k.slice(k.indexOf(':') + 1);
      if (!built.has(bare)) built.set(bare, k);
    }
    byBareId.set(keys, built);
    m = built;
  }
  return m.get(id) as K | undefined;
}

/** The peer that owns a key, or the peer itself when given a bare peer id. */
export function ownerOf(keyOrPeer: string): PeerId {
  const i = keyOrPeer.indexOf(':');
  return peerId(i < 0 ? keyOrPeer : keyOrPeer.slice(0, i));
}
