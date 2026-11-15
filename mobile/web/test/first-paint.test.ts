// First paint from the phone (ADR-0065 §4): rows, cursors and drafts come from IndexedDB for every saved computer
// before the network. With no network at all, 4,000 cached tasks are on screen as soon as start() returns, each
// computer asks from its saved cursor, and the bar says it's offline with when it was last updated.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { performance } from 'node:perf_hooks';
import { createClient, indexedDBPersist, peerId } from '@wisp/client';
import type { Reached } from '../src/bridge.ts';
import { phonePeers, PHONE } from '../src/peers.ts';
import { fakeBridge, fakeIndexedDB, fakeServer, fakeTimers, settle, ssh, welcome } from './helpers.ts';

const PER = 2_000;
const rows = (peer: string) => Array.from({ length: PER }, (_, i) => ({ e: 'session', id: `s${i}`, v: { id: `s${i}`, title: `Task ${i} on ${peer}`, state: 'review', updatedAt: i } }));

test('four_thousand_cached_tasks_paint_before_the_network', async () => {
  const db = fakeIndexedDB();
  const computers = [ssh('a', { peer: 'studio', name: 'Studio' }), ssh('b', { peer: 'laptop', name: 'Laptop' })];
  const clock = fakeTimers();

  // an earlier run: both computers online, their lists synced, a draft typed
  {
    const bridge = fakeBridge(computers);
    bridge.answers = [{ kind: 'ssh', port: 5001, token: 't', name: 'x' }];
    const srv = fakeServer();
    const persist = indexedDBPersist(db.idb, db.range);
    const client = createClient({ clientId: 'phone-1', build: 'test', home: PHONE, width: 390, timers: clock, persist });
    await client.start(await phonePeers(bridge, undefined, { fetch: srv.fetch }));
    for (const p of ['studio', 'laptop']) client.peers.watch(peerId(p), 'list', client.scope);
    await settle(clock);
    for (const [i, p] of ['studio', 'laptop'].entries()) {
      srv.streams[i]?.frame(5, welcome(p));
      srv.streams[i]?.frame(7, { t: 'snapshot', topic: 'list', seq: 7, rows: rows(p) });
    }
    await settle(clock);
    client.drafts.set('studio:s1', 'prompt', 'half a thought');
    assert.equal(client.entities.keys('session')().length, 2 * PER);
    await client.flush();
    client.dispose();
  }

  // this run: no network. The bridge never answers, so no stream ever opens.
  clock.advance(60 * 60_000);
  const bridge = fakeBridge(computers);
  bridge.connect = (id) => {
    bridge.connects.push(id);
    return new Promise<Reached>(() => {});
  };
  const persist = indexedDBPersist(db.idb, db.range);
  const client = createClient({ clientId: 'phone-1', build: 'test', home: PHONE, width: 390, timers: clock, persist });
  const t0 = performance.now();
  await client.start(await phonePeers(bridge));
  const ms = performance.now() - t0;
  assert.equal(client.entities.keys('session')().length, 2 * PER, 'every cached task, before any connect');
  assert.equal(client.entities.peek<{ title: string }>('session', 'laptop:s1999' as never)?.title, 'Task 1999 on laptop');
  assert.equal(client.drafts.get('studio:s1', 'prompt')(), 'half a thought', 'drafts too');
  assert.deepEqual(bridge.connects, [], 'painted first: the connects come after');
  assert.ok(ms < 800, `the list's rows were ready in ${ms.toFixed(0)} ms (budget 800 ms on the reference phone)`);
  await settle(clock);
  assert.deepEqual(bridge.connects.sort(), ['a', 'b'], 'then every saved computer, from its cursor');
  // still nothing: two keepalives on, the silent connect is given up and the bar says since when
  clock.advance(30_000);
  await settle(clock);
  const st = client.peers.status(peerId('studio'))();
  assert.equal(st.kind, 'unreachable');
  assert.ok(st.kind === 'unreachable' && typeof st.heardAt === 'number' && st.heardAt < clock.now() - 60 * 60_000, 'last updated: the earlier run');
  client.dispose();
});
