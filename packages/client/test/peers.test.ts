import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Notice, Topic } from '@wisp/protocol';
import { createEntityStore } from '../src/entities.ts';
import { compareVersions, hasFeature } from '../src/features.ts';
import { entityKey, peerId } from '../src/ids.ts';
import { KEEP_UNWATCHED_MS, createPeers } from '../src/peers.ts';
import { createScope } from '../src/scope.ts';
import { PeerDown } from '../src/transport.ts';
import { fakeTimers, type FakeTimers, fakeTransport, peerInfo, tick, welcome } from './helpers.ts';

const A = peerId('laptop');

let clockNow: FakeTimers | undefined; // the running test's clock
/** lets streams deliver and zero-delay timers (a peer's first open) run */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    clockNow?.advance(0);
    await tick();
  }
}

async function setup() {
  const clock = fakeTimers(1_700_000_000_000);
  clockNow = clock;
  const scope = createScope('test', clock);
  const { reader, writer } = createEntityStore();
  const peers = createPeers({ scope, writer, clientId: 'c1', build: 'test' });
  const t = fakeTransport();
  return { clock, scope, reader, writer, peers, t, conn: () => t.connections.at(-1) };
}

// ---------- features come from the peer ----------

test('hasFeature answers from the welcome, with a reason when the peer lacks it', () => {
  const info = peerInfo({ name: 'studio-mac', plugins: [{ id: 'wisp.crew', version: '2.3.0', commands: ['crew.hire'], topics: ['crew'], blocks: ['crew.card'] }] });
  assert.deepEqual(hasFeature(info, { plugin: 'wisp.crew', command: 'crew.hire' }), { ok: true });
  assert.deepEqual(hasFeature(info, { plugin: 'wisp.crew', minVersion: '2.4', label: 'Crew' }), { ok: false, reason: 'Crew needs wisp.crew 2.4 on studio-mac' });
  assert.deepEqual(hasFeature(info, { plugin: 'wisp.crew', command: 'crew.fire', label: 'Firing' }), { ok: false, reason: 'Firing needs a newer wisp.crew on studio-mac' });
  assert.deepEqual(hasFeature(info, { plugin: 'wisp.crew', block: 'crew.card' }), { ok: true });
  assert.deepEqual(hasFeature(info, { plugin: 'acme.thing', label: 'Thing' }), { ok: false, reason: "Thing isn't installed on studio-mac" });
  assert.deepEqual(hasFeature(undefined, { plugin: 'wisp.crew' }, 'studio-mac'), { ok: false, reason: "studio-mac hasn't connected yet" });
});

test('compareVersions: numeric parts, missing parts are zero, a pre-release sorts first', () => {
  assert.equal(compareVersions('2.10.0', '2.9.9'), 1);
  assert.equal(compareVersions('2.4', '2.4.0'), 0);
  assert.equal(compareVersions('2.4.0-rc.1', '2.4.0'), -1);
  assert.equal(compareVersions('v3', '2.99'), 1);
});

test('the peer manager learns features from the welcome', async () => {
  const { peers, t, conn } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  assert.equal(peers.hasFeature(A, { plugin: 'wisp.runtime' }).ok, false, 'before the welcome');
  conn()?.push(welcome());
  await settle();
  assert.equal(peers.status(A)().kind, 'online');
  assert.deepEqual(peers.hasFeature(A, { plugin: 'wisp.runtime', command: 'session.rename' }), { ok: true });
  assert.equal(peers.info(A)()?.version, '2.0.0');
});

// A phone saved its grid box as "Cloud"; the box called itself by its container id ("1959efa12b19") and that's what
// the phone showed. The name this device gave a computer wins; unnamed, the computer's own.
test('a peer goes by the name this device gave it, else by its own', async () => {
  const { peers, t, conn } = await setup();
  peers.add({ peer: A, name: 'Cloud', transport: t });
  const B = peerId('home');
  peers.add({ peer: B, name: '', transport: fakeTransport() });
  await settle();
  conn()?.push(welcome({}, { name: '1959efa12b19' }));
  await settle();
  assert.equal(peers.name(A), 'Cloud');
  assert.equal(peers.name(B), 'home', 'no name yet: its id');
});

// ---------- status ----------

test('a peer outside the protocol window is incompatible, and says which side to update', async () => {
  for (const [range, update, reason] of [
    [{ proto_min: 1, proto_max: 1 }, 'peer', 'Update wisp on studio-mac'],
    [{ proto_min: 9, proto_max: 9 }, 'app', 'Update the app to use studio-mac'],
  ] as const) {
    const { peers, t, conn, clock } = await setup();
    peers.add({ peer: A, name: '', transport: t }); // unnamed here: the peer's own name
    await settle();
    conn()?.push(welcome({}, { ...range, name: 'studio-mac' }));
    await settle();
    assert.deepEqual(peers.status(A)(), { kind: 'incompatible', reason, update, ...(update === 'peer' && { command: 'npm install -g wisp-agents@latest' }) });
    clock.advance(600_000);
    await settle();
    assert.equal(t.connections.length, 1, 'no retries against an incompatible peer');
  }
});

test('a_silent_stream_is_dropped_and_reopened_from_its_cursor', async () => {
  const { peers, t, conn, clock } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  conn()?.frame(4, welcome());
  await settle();
  const heard = clock.now();
  // heartbeats (empty frames) keep it: a minute of them and nothing is dropped
  for (let i = 0; i < 4; i++) {
    clock.advance(15_000);
    conn()?.frame(null);
    await settle();
  }
  assert.equal(t.connections.length, 1);
  assert.equal(conn()?.aborted, false);
  // then nothing for two keepalives: the stream is dropped, the bar says when it was last heard
  clock.advance(30_000);
  await settle();
  assert.equal(conn()?.aborted, true, 'the dead stream is closed, not left hanging');
  const st = peers.status(A)();
  assert.equal(st.kind, 'unreachable');
  assert.equal(st.kind === 'unreachable' && st.heardAt, heard + 60_000, 'the last heartbeat');
  assert.equal(st.kind === 'unreachable' && st.reason, 'no answer', 'why: not an older error');
  clock.advance(2_000);
  await settle();
  assert.equal(t.connections.length, 2, 'reopened after the first wait');
  assert.equal(conn()?.params['cursor'], '4', 'from its cursor: only what changed comes');
});

test('reconnect_skips_the_backoff_and_never_wakes_a_sleeping_box', async () => {
  const { peers, t, conn, clock } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  conn()?.end(new Error('ECONNREFUSED'));
  await settle();
  clock.advance(2_000);
  await settle();
  conn()?.end(new Error('ECONNREFUSED'));
  await settle(); // now waiting 5 s
  assert.equal(t.connections.length, 2);
  peers.reconnect();
  await settle();
  assert.equal(t.connections.length, 3, 'now, not in 5 s');
  assert.equal(peers.status(A)().kind, 'connecting');
  conn()?.push(welcome());
  await settle();
  clock.advance(5_000);
  await settle();
  assert.equal(t.connections.length, 3, 'the old wait was cancelled');
  // live: Reconnect replaces a stream that may be dead, resuming, with no offline flash
  const seen: string[] = [];
  peers.reconnect(A);
  seen.push(peers.status(A)().kind);
  await settle();
  assert.deepEqual(seen, ['connecting']);
  assert.equal(t.connections[2]?.aborted, true);
  assert.equal(t.connections.length, 4);
  // a sleeping box stays asleep
  const box = fakeTransport();
  const B = peerId('box');
  peers.add({ peer: B, name: 'box', transport: box });
  await settle();
  box.connections[0]?.end(new PeerDown('asleep', 'asleep'));
  await settle();
  assert.equal(peers.status(B)().kind, 'asleep');
  peers.reconnect();
  await settle();
  assert.equal(box.connections.length, 1);
  assert.equal(box.wakes, 0);
});

test('an unreachable peer says why and is retried with backoff', async () => {
  const { peers, t, conn, clock } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  conn()?.end(new Error('ECONNREFUSED'));
  await settle();
  assert.deepEqual(peers.status(A)(), { kind: 'unreachable', reason: 'ECONNREFUSED' });
  clock.advance(1_999); // v1's first wait: 2 s
  await settle();
  assert.equal(t.connections.length, 1);
  clock.advance(1);
  await settle();
  assert.equal(t.connections.length, 2);
  conn()?.push(welcome());
  await settle();
  assert.equal(peers.status(A)().kind, 'online');
});

test('a sleeping box stays asleep: no retries, a passive watch does not wake it, opening its task does', async () => {
  const { peers, t, clock, scope } = await setup();
  t.failNext = new PeerDown('asleep', 'asleep');
  peers.add({ peer: A, name: 'box', transport: t });
  await settle();
  assert.equal(peers.status(A)().kind, 'asleep');
  clock.advance(600_000);
  await settle();
  assert.equal(t.connections.length, 0, 'not retried');
  peers.watch(A, 'list', scope, { passive: true });
  await settle();
  assert.equal(t.wakes, 0, 'a passive subscription never counts as use');
  peers.watch(A, 'session:s1', scope);
  await settle();
  assert.equal(t.wakes, 1);
  assert.equal(peers.status(A)().kind, 'waking');
  t.connections.at(-1)?.push(welcome());
  await settle();
  assert.equal(peers.status(A)().kind, 'online');
});

test('a command for a sleeping box wakes it', async () => {
  const { peers, t } = await setup();
  t.failNext = new PeerDown('asleep', 'asleep');
  peers.add({ peer: A, name: 'box', transport: t });
  await settle();
  peers.mutate(A, { mid: 'm1', cmd: 'session.followUp', args: {} });
  await settle();
  assert.equal(t.wakes, 1);
  t.connections.at(-1)?.push(welcome());
  await settle();
  assert.deepEqual(t.posted.filter((m) => m.t === 'mutate').map((m) => (m.t === 'mutate' ? m.mid : '')), ['m1']);
});

// ---------- subscriptions ----------

test('two views of one task share one subscription; the last to go ends it', async () => {
  const { peers, t, conn, scope } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  conn()?.push(welcome());
  await settle();
  const topic: Topic = 'session:s1';
  const v1 = scope.child('view1');
  const v2 = scope.child('view2');
  peers.watch(A, topic, v1);
  peers.watch(A, topic, v2);
  const subs = () => t.posted.filter((m) => m.t === 'sub' && m.topic === topic).length;
  const unsubs = () => t.posted.filter((m) => m.t === 'unsub' && m.topic === topic).length;
  assert.equal(subs(), 1);
  v1.dispose();
  assert.equal(unsubs(), 0);
  v2.dispose();
  await settle(); // queued behind the sub's POST, which is still in flight (FIFO per peer)
  assert.equal(unsubs(), 1);
});

test('back-to-back posts on a peer stay strictly FIFO while a previous post is in flight', async () => {
  const { peers, t, conn, scope } = await setup();
  const order: string[] = [];
  const gates: (() => void)[] = [];
  const origPost = t.post.bind(t);
  t.post = (msg, sig) => {
    order.push(`${msg.t}:${'topic' in msg ? msg.topic : ''}`);
    void origPost(msg, sig);
    return new Promise<void>((resolve) => {
      gates.push(resolve);
    });
  };
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  conn()?.push(welcome());
  await settle();
  const topic: Topic = 'session:fifo';
  const v1 = scope.child('v1');
  peers.watch(A, topic, v1);
  v1.dispose();
  // `sub` started synchronously; `unsub` waits until `sub` settles
  assert.deepEqual(order, ['sub:session:fifo']);
  gates.shift()?.();
  await settle();
  assert.deepEqual(order, ['sub:session:fifo', 'unsub:session:fifo']);
  gates.shift()?.();
});

test('a_closed_tasks_rows_leave_memory_a_while_after_its_last_watch_and_a_reopen_by_then_keeps_them (perf r2 #14)', async () => {
  const { peers, t, conn, scope, reader, clock } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  peers.watch(A, 'list', scope);
  await settle();
  conn()?.push(welcome());
  await settle();
  conn()?.frame(3, { t: 'snapshot', topic: 'list', seq: 3, rows: [{ e: 'session', id: 's1', v: { title: 'one' } }] });
  await settle();
  const topic: Topic = 'session:s1';
  const open = (): ReturnType<typeof scope.child> => {
    const v = scope.child('task');
    peers.watch(A, topic, v);
    return v;
  };
  let view = open();
  await settle();
  conn()?.frame(4, {
    t: 'snapshot',
    topic,
    seq: 4,
    rows: [
      { e: 'session', id: 's1', v: { title: 'one' } },
      { e: 'event', id: 'e1', v: { seq: 1, turn_id: 't1', session_id: 's1' } },
    ],
  });
  await settle();
  // and what streams in after the snapshot: an append under the task's turn (its events as one list row)
  conn()?.frame(5, { t: 'patch', seq: 5, ops: [{ topic, e: 'event', id: 't1', op: 'append', v: [{ seq: 2, turn_id: 't1', session_id: 's1' }] }] });
  await settle();
  const event = (): unknown => reader.peek('event', entityKey(A, 'e1'));
  const streamed = (): unknown => reader.peek('event', entityKey(A, 't1'));
  assert.ok(event(), 'the open task holds its events');
  assert.ok(streamed(), 'and what streamed in');
  // closed, and opened again before the linger ends: still there, nothing to fetch again
  view.dispose();
  clock.advance(KEEP_UNWATCHED_MS - 1);
  view = open();
  clock.advance(KEEP_UNWATCHED_MS * 2);
  assert.ok(event(), 'a reopen within the linger keeps the rows');
  // closed for good: its events go, its row stays (the list holds it too), and the topic no longer counts as loaded
  view.dispose();
  clock.advance(KEEP_UNWATCHED_MS);
  assert.equal(event(), undefined, 'nothing watches it: out of memory');
  assert.equal(streamed(), undefined, 'what streamed in too');
  assert.deepEqual(reader.peek('session', entityKey(A, 's1')), { title: 'one' }, 'the list still holds the row');
  assert.equal(reader.loaded(A, topic)(), false);
});

test('frames reach the store, rows from every peer side by side, notices reach their listeners', async () => {
  const { peers, scope, reader } = await setup();
  const notices: Notice[] = [];
  peers.onNotice((_p, n) => notices.push(n));
  const boxes = ['laptop', 'box'].map((name) => {
    const t = fakeTransport();
    peers.add({ peer: peerId(name), name, transport: t });
    peers.watch(peerId(name), 'list', scope);
    return t;
  });
  await settle();
  for (const [i, t] of boxes.entries()) {
    t.connections[0]?.push(welcome({}, { peer: i === 0 ? 'laptop' : 'box' }));
    await settle();
    t.connections[0]?.frame(3, { t: 'snapshot', topic: 'list', seq: 3, rows: [{ e: 'session', id: 's1', v: { from: i } }] });
  }
  boxes[1]?.connections[0]?.push({ t: 'notice', kind: 'toast', data: { text: 'done' } });
  await settle();
  assert.deepEqual(reader.peek('session', entityKey(peerId('laptop'), 's1')), { from: 0 });
  assert.deepEqual(reader.peek('session', entityKey(peerId('box'), 's1')), { from: 1 });
  assert.equal(reader.list('session')().length, 2);
  assert.deepEqual(notices.map((n) => n.kind), ['toast']);
  assert.equal(peers.save(peerId('box'))?.cursor, 3, 'the cursor to persist is the last frame id');
});

test('removing a peer ends its connection and its scope', async () => {
  const { peers, t, conn } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  await settle();
  peers.remove(A);
  assert.equal(conn()?.aborted, true);
  assert.deepEqual(peers.list()(), []);
  peers.add({ peer: A, name: 'again', transport: t }); // a removed peer can come back
  assert.throws(() => peers.add({ peer: A, name: 'x', transport: t }), /twice/);
});

test('removing_a_peer_drops_its_rows_so_a_re_keyed_computer_is_never_listed_twice', async () => {
  const { peers, t, conn, scope, reader } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  peers.watch(A, 'config', scope);
  await settle();
  conn()?.push(welcome());
  await settle();
  conn()?.frame(3, { t: 'snapshot', topic: 'config', seq: 3, rows: [{ e: 'project', id: 'wisp', v: { name: 'wisp', path: '/code/wisp' } }] });
  await settle();
  assert.equal(reader.list('project')().length, 1);
  peers.remove(A);
  assert.deepEqual(reader.list('project')(), [], 'the removed peer’s project is gone');
  assert.equal(reader.peek('project', entityKey(A, 'wisp')), undefined);
});

test('an op the store cannot apply starts its topic over: the link unsubscribes and subscribes afresh', async () => {
  const { peers, t, conn, scope } = await setup();
  peers.add({ peer: A, name: 'laptop', transport: t });
  peers.watch(A, 'session:s9', scope);
  await settle();
  conn()?.push(welcome());
  await settle();
  conn()?.frame(3, { t: 'snapshot', topic: 'session:s9', seq: 3, rows: [] }); // the link takes ops once a topic has its snapshot
  await settle();
  const before = t.posted.length;
  conn()?.frame(4, { t: 'patch', seq: 4, ops: [{ topic: 'session:s9', e: 'session', id: 's9', op: 'patch', ops: [{ op: 'replace', path: '/title', value: 'x' }] }] });
  await settle();
  assert.deepEqual(t.posted.slice(before).map((m) => `${m.t} ${'topic' in m ? m.topic : ''}`), ['unsub session:s9', 'sub session:s9']);
});

test('a_watch_on_a_peer_added_later_starts_when_it_is_added_and_one_whose_scope_ended_never_does', async () => {
  const { peers, t, conn, scope } = await setup();
  const B = peerId('sandbox-m1');
  const view = scope.child('task');
  const gone = scope.child('closed-view');
  peers.watch(B, 'session:t1' as Topic, view);
  peers.watch(B, 'session:t2' as Topic, gone);
  gone.dispose();
  peers.add({ peer: B, name: 'Sandbox', transport: t });
  await settle();
  conn()?.push(welcome());
  await settle();
  // it rides the hello (the topics a connection opens with) or a `sub` after it
  const said = JSON.stringify([t.connections.map((c) => c.params), t.posted]);
  assert.ok(said.includes('session:t1'), said);
  assert.ok(!said.includes('session:t2'), 'its view had closed');
});

test('a_peer_that_refuses_every_write_is_not_reopened_in_a_loop (grid box 403 on the phone, 2026-11-23)', async () => {
  const { peers, t, conn, clock } = await setup();
  // the box answered every sync POST from the phone's webview `forbidden origin`: each failure reopened the stream at
  // once, the new welcome sent the subs again, and the computer flipped online/offline ~4 times a second
  t.post = () => Promise.reject(new PeerDown('unreachable', 'sub: HTTP 403'));
  peers.add({ peer: A, name: 'laptop', transport: t });
  // a write waiting for an answer is sent again after every welcome
  peers.mutate(A, { mid: 'm1', cmd: 'session.markRead', args: {} });
  await settle();
  conn()?.push(welcome());
  await settle();
  assert.equal(t.connections.length, 2, 'the first failure reopens at once (a stream the peer dropped)');
  conn()?.push(welcome());
  await settle();
  clock.advance(1_999);
  await settle();
  assert.equal(t.connections.length, 2, 'failing again after the reopen: it waits out the backoff');
  assert.equal(peers.status(A)().kind, 'online', 'the open stream still reads: no offline flash');
  clock.advance(1);
  await settle();
  assert.equal(t.connections.length, 3, 'reopened once the first wait is over');
  conn()?.push(welcome());
  await settle();
  clock.advance(4_999);
  await settle();
  assert.equal(t.connections.length, 3, 'and the next wait is longer');
  clock.advance(1);
  await settle();
  assert.equal(t.connections.length, 4);
  // writes get through again: the next failure is a first one
  t.post = async (m) => void t.posted.push(m);
  conn()?.push(welcome());
  await settle();
  assert.equal(t.connections.length, 4, 'nothing to reopen');
});
