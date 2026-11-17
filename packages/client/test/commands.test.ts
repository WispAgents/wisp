import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Mutate } from '@wisp/protocol';
import { CommandError, createCommands, defineCommand, waitingToSend } from '../src/commands.ts';
import { createEntityStore } from '../src/entities.ts';
import { entityKey, peerId } from '../src/ids.ts';
import { createPeers } from '../src/peers.ts';
import { memoryPersist, type Persist } from '../src/persist.ts';
import { createScope } from '../src/scope.ts';
import { untracked } from '../src/signals.ts';
import type { EntityKey } from '../src/types.ts';
import { fakeTimers, type FakeTimers, fakeTransport, tick, welcome } from './helpers.ts';
import { createClient } from '../src/client.ts';

const A = peerId('laptop');
const S1 = entityKey(A, 's1');

const rename = defineCommand<{ id: EntityKey; title: string }>({
  id: 'session.rename',
  owner: (a) => a.id,
  optimistic: (base, a) => {
    const row = base.peek<{ title: string }>('session', a.id);
    return row ? [{ e: 'session', id: a.id, op: 'patch', ops: [{ op: 'replace', path: '/title', value: a.title }] }] : [];
  },
});
const followUp = defineCommand<{ id: EntityKey; text: string }>({ id: 'session.followUp', owner: (a) => a.id, optimistic: 'none', pending: 'the session.followUp control' });

let clockNow: FakeTimers | undefined; // the running test's clock
/** lets streams deliver and zero-delay timers (a peer's first open) run */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    clockNow?.advance(0);
    await tick();
  }
}

async function setup(persist: Persist = memoryPersist(), opts: { connect?: boolean } = {}) {
  const clock = fakeTimers(1_700_000_000_000);
  clockNow = clock;
  const scope = createScope('test', clock);
  const { reader, writer } = createEntityStore();
  const peers = createPeers({ scope, writer, clientId: 'c1', build: 'test' });
  const errors: CommandError[] = [];
  const agains: ((() => void) | undefined)[] = [];
  const commands = createCommands({
    reader,
    writer,
    peers,
    persist,
    now: clock.now,
    onError: (e, _cmd, again) => {
      errors.push(e);
      agains.push(again);
    },
  });
  const t = fakeTransport();
  peers.add({ peer: A, name: 'laptop', transport: t });
  peers.watch(A, 'list', scope);
  await settle();
  const conn = () => {
    const c = t.connections.at(-1);
    assert.ok(c, 'a connection is open');
    return c;
  };
  if (opts.connect !== false) {
    conn().push(welcome());
    await settle();
    conn().frame(1, { t: 'snapshot', topic: 'list', seq: 1, rows: [{ e: 'session', id: 's1', v: { title: 'old' } }] });
    await settle();
    assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'old');
  }
  const mutates = () => t.posted.filter((m): m is Mutate => m.t === 'mutate');
  return { clock, scope, reader, writer, peers, commands, errors, agains, t, conn, mutates, persist };
}

test('a command shows its optimistic patch at once, and the ack keeps it as the server state', async () => {
  const { reader, commands, conn, mutates } = await setup();
  const done = commands.run(rename, { id: S1, title: 'new' });
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'new', 'before any answer');
  const [m] = mutates();
  assert.ok(m);
  assert.match(m.mid, /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/, 'the mid is a UUIDv7');
  assert.equal(m.cmd, 'session.rename');
  conn().frame(7, { t: 'patch', seq: 7, ops: [{ topic: 'list', e: 'session', id: 's1', op: 'patch', ops: [{ op: 'replace', path: '/title', value: 'new' }] }] }, { t: 'ack', mid: m.mid, seq: 7, result: { ok: 1 } });
  await settle();
  assert.deepEqual(await done, { ok: 1 });
  assert.equal(reader.pending()().length, 0);
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'new');
});

test('a reject rolls the optimistic patch back and calls onError (the toast)', async () => {
  const { reader, commands, conn, mutates, errors } = await setup();
  const done = commands.run(rename, { id: S1, title: 'bad' });
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'bad');
  conn().push({ t: 'reject', mid: mutates()[0]?.mid ?? '', code: 'vetoed', message: 'titles are locked' });
  await settle();
  await assert.rejects(done, (e: unknown) => e instanceof CommandError && e.code === 'vetoed');
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'old');
  assert.deepEqual(errors.map((e) => e.message), ['titles are locked']);
});

test('a refused command hands onError a way to run it again, with the same arguments and patch (TRY-15)', async () => {
  const { reader, commands, conn, mutates, agains } = await setup();
  void commands.run(rename, { id: S1, title: 'later' }).catch(() => {});
  conn().push({ t: 'reject', mid: mutates()[0]?.mid ?? '', code: 'timeout', message: 'the peer was busy' });
  await settle();
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'old', 'rolled back');
  const again = agains[0];
  assert.ok(again, 'Try again is offered');
  again();
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'later', 'the optimistic patch is back at once');
  const [first, second] = mutates();
  assert.ok(first && second && first.mid !== second.mid, 'a new mid: a new command, not a resend');
  assert.deepEqual([second.cmd, second.args], ['session.rename', { id: S1, title: 'later' }]);
});

test('a command can not run twice: the same run while in flight is the first run (#643)', async () => {
  const { commands, mutates, conn } = await setup();
  const a = commands.run(followUp, { id: S1, text: 'go' });
  const b = commands.run(followUp, { id: S1, text: 'go' });
  assert.equal(a, b);
  assert.equal(mutates().length, 1);
  conn().push({ t: 'ack', mid: mutates()[0]?.mid ?? '', seq: 0 });
  await settle();
  await a;
  // once answered, the same command is a new run
  void commands.run(followUp, { id: S1, text: 'go' });
  assert.equal(mutates().length, 2);
  assert.notEqual(mutates()[0]?.mid, mutates()[1]?.mid);
});

test('offline: the command waits, then goes after the welcome; a lost stream resends it with the same mid', async () => {
  const { commands, mutates, conn, clock, reader, writer } = await setup(memoryPersist(), { connect: false });
  writer.applyServer(A, [{ topic: 'list', e: 'session', id: 's1', op: 'upsert', v: { title: 'old' } }]);
  const done = commands.run(rename, { id: S1, title: 'offline' });
  assert.equal(reader.peek<{ title: string }>('session', S1)?.title, 'offline', 'shown at once, even offline');
  assert.equal(mutates().length, 0);
  assert.equal(commands.pending()()[0]?.waiting, true, 'waiting for the peer');
  conn().push(welcome());
  await settle();
  assert.equal(mutates().length, 1);
  assert.equal(commands.pending()()[0]?.waiting, false);
  const mid = mutates()[0]?.mid;
  conn().end(new Error('tunnel dropped'));
  await settle();
  clock.advance(2_000); // the link's first retry
  await settle();
  conn().push(welcome({ resume: true }));
  await settle();
  assert.equal(mutates().length, 2);
  assert.equal(mutates()[1]?.mid, mid, 'the same idempotency key: the peer runs it once');
  conn().push({ t: 'ack', mid: mid ?? '', seq: 0 });
  await settle();
  await done;
});

test('pending commands survive a reload: the overlay is back on first paint and resent with the same mid', async () => {
  const persist = memoryPersist();
  const first = await setup(persist, { connect: false });
  first.writer.applyServer(A, [{ topic: 'list', e: 'session', id: 's1', op: 'upsert', v: { title: 'old' } }]);
  void first.commands.run(rename, { id: S1, title: 'kept' });
  await settle();
  first.scope.dispose();
  const saved = (await persist.entries('pending'))[0]?.[1] as { mid: string } | undefined;
  assert.ok(saved);

  const second = await setup(persist, { connect: false });
  second.writer.applyServer(A, [{ topic: 'list', e: 'session', id: 's1', op: 'upsert', v: { title: 'old' } }]);
  await second.commands.restore();
  assert.equal(second.reader.peek<{ title: string }>('session', S1)?.title, 'kept');
  // running it again joins the restored one instead of sending a second
  void second.commands.run(rename, { id: S1, title: 'kept' });
  second.conn().push(welcome());
  await settle();
  assert.deepEqual(second.mutates().map((m) => m.mid), [saved.mid]);
  second.conn().push({ t: 'ack', mid: saved.mid, seq: 0 });
  await settle();
  assert.deepEqual(await persist.entries('pending'), [], 'the answer clears it from disk');
});

test('a peer without the feature refuses the command at once, with the reason, and sends nothing', async () => {
  const { commands, mutates, errors } = await setup();
  const crew = defineCommand<{ peer: string }>({ id: 'crew.hire', owner: (a) => a.peer, optimistic: 'none', pending: 'the crew.hire control', feature: { plugin: 'wisp.crew', label: 'Crew' } });
  await assert.rejects(commands.run(crew, { peer: 'laptop' }), /Crew isn't installed on laptop/);
  assert.equal(mutates().length, 0);
  assert.equal(errors[0]?.code, 'unavailable');
});

test('an optimistic patch that throws fails the command as invalid instead of breaking the store', async () => {
  const { commands, reader } = await setup();
  const broken = defineCommand<{ id: EntityKey }>({
    id: 'x.broken',
    owner: (a) => a.id,
    optimistic: () => {
      throw new Error('no such row');
    },
  });
  await assert.rejects(commands.run(broken, { id: S1 }), (e: unknown) => e instanceof CommandError && e.code === 'invalid');
  assert.equal(reader.pending()().length, 0);
});

test('commands route to the peer named in the id (origin routing)', async () => {
  const { peers, commands, scope } = await setup();
  const box = fakeTransport();
  peers.add({ peer: peerId('box'), name: 'box', transport: box });
  await settle();
  box.connections[0]?.push(welcome({}, { peer: 'box', name: 'box' }));
  await settle();
  void commands.run(followUp, { id: entityKey(peerId('box'), 's9'), text: 'hi' });
  assert.equal(box.posted.filter((m) => m.t === 'mutate').length, 1);
  scope.dispose();
});

test('the type requires optimistic: a command without one does not compile', () => {
  // @ts-expect-error: optimistic is required (a patch or 'none')
  defineCommand<{ id: string }>({ id: 'x.y', owner: (a) => a.id });
  assert.ok(true);
});

test('a command with no optimistic result names the control that shows it in flight (design/feedback.md)', () => {
  // @ts-expect-error `optimistic: 'none'` alone: nothing on screen would answer the action
  const bare = defineCommand<{ id: string }>({ id: 'x.y', owner: (a) => a.id, optimistic: 'none' });
  const named = defineCommand<{ id: string }>({ id: 'x.y', owner: (a) => a.id, optimistic: 'none', pending: 'the Ship it button' });
  assert.equal(bare.optimistic, 'none');
  assert.equal(named.pending, 'the Ship it button');
});

test('a command\'s wire shape: the peer gets the args as its command takes them (an entity key as the bare id)', async () => {
  const { commands, mutates } = await setup();
  const view = defineCommand<{ key: EntityKey }>({ id: 'ship.view', owner: (a) => a.key, wire: (a) => ({ session: a.key.slice(a.key.indexOf(':') + 1) }), optimistic: 'none', pending: 'the ship.view control' });
  void commands.run(view, { key: S1 }).catch(() => undefined);
  assert.deepEqual(mutates()[0]?.args, { session: 's1' });
});

const markDone = defineCommand<{ id: EntityKey }>({
  id: 'session.done',
  owner: (a) => a.id,
  optimistic: (base, a) => (base.peek('session', a.id) ? [{ e: 'session', id: a.id, op: 'patch', ops: [{ op: 'replace', path: '/state', value: 'done' }] }] : []),
});
const online = (peers: { status(p: typeof A): () => { kind: string } }) => (p: typeof A) => untracked(peers.status(p)).kind === 'online';

test('an_offline_command_is_sent_once_on_reconnect', async () => {
  const { commands, mutates, conn, clock, reader, writer, peers, errors } = await setup(memoryPersist(), { connect: false });
  writer.applyServer(A, [{ topic: 'list', e: 'session', id: 's1', op: 'upsert', v: { title: 't', state: 'review' } }]);
  conn().end(new Error('no network'));
  await settle();
  // Mark done offline: done at once, and the row says Waiting to send
  const done = commands.run(markDone, { id: S1 });
  assert.equal(reader.peek<{ state: string }>('session', S1)?.state, 'done', 'at once');
  assert.deepEqual([...waitingToSend(untracked(reader.pending()), online(peers))], [S1]);
  // the peer stays away through two retries: nothing is sent, nothing is lost
  for (const wait of [2_000, 5_000]) {
    clock.advance(wait);
    await settle();
    conn().end(new Error('no network'));
    await settle();
  }
  assert.equal(mutates().length, 0);
  // pull to refresh: back now, and the command goes once
  peers.reconnect();
  await settle();
  conn().push(welcome());
  await settle();
  conn().frame(2, { t: 'snapshot', topic: 'list', seq: 2, rows: [{ e: 'session', id: 's1', v: { title: 't', state: 'review' } }] });
  await settle();
  assert.equal(mutates().length, 1);
  assert.equal(reader.peek<{ state: string }>('session', S1)?.state, 'done', 'still done over the fresh rows');
  assert.equal(waitingToSend(untracked(reader.pending()), online(peers)).size, 0, 'sent: no longer waiting');
  for (let i = 0; i < 8; i++) {
    clock.advance(15_000);
    conn().frame(null); // heartbeats meanwhile
    await settle();
  }
  assert.equal(mutates().length, 1, 'once: no resend while the stream lives');
  // a later refusal rolls it back with the toast
  conn().push({ t: 'reject', mid: mutates()[0]?.mid ?? '', code: 'vetoed', message: 'already merged' });
  await settle();
  await assert.rejects(done, (e: unknown) => e instanceof CommandError && e.code === 'vetoed');
  assert.equal(reader.peek<{ state: string }>('session', S1)?.state, 'review');
  assert.deepEqual(errors.map((e) => e.message), ['already merged']);
});

test('pending_commands_survive_a_restart', async () => {
  // the app killed with a command waiting: the next start shows it and sends it, once, with the same key
  const persist = memoryPersist();
  const timers = fakeTimers(1_700_000_000_000);
  clockNow = timers;
  const boot = async () => {
    const t = fakeTransport();
    const client = createClient({ clientId: 'c1', build: 'test', home: A, width: 1200, persist, timers });
    await client.start([{ peer: A, name: 'laptop', transport: t }]);
    client.peers.watch(A, 'list', client.scope); // the list on screen
    await settle();
    return { client, t };
  };
  const first = await boot();
  first.t.connections[0]?.push(welcome());
  await settle();
  first.t.connections[0]?.frame(3, { t: 'snapshot', topic: 'list', seq: 3, rows: [{ e: 'session', id: 's1', v: { title: 't', state: 'review' } }] });
  await settle();
  first.t.connections[0]?.end(new Error('no network'));
  await settle();
  void first.client.commands.run(markDone, { id: S1 }).catch(() => undefined);
  await first.client.flush();
  first.client.dispose(); // killed: no answer ever came
  assert.equal(first.t.posted.filter((m) => m.t === 'mutate').length, 0);

  const second = await boot();
  assert.equal(second.client.entities.peek<{ state: string }>('session', S1)?.state, 'done', 'the overlay is back on first paint');
  assert.equal(untracked(second.client.commands.pending()).length, 1);
  const c = second.t.connections[0];
  assert.equal(c?.params['cursor'], '3', 'resumed from the saved cursor');
  c?.push(welcome({ resume: true, epoch: 'e1' }));
  await settle();
  const sent = second.t.posted.filter((m): m is Mutate => m.t === 'mutate');
  const saved = (await persist.entries('pending'))[0]?.[1] as { mid: string } | undefined;
  assert.deepEqual(sent.map((m) => m.mid), [saved?.mid], 'sent once, with the key it was saved with');
  c?.push({ t: 'ack', mid: saved?.mid ?? '', seq: 4 });
  await settle();
  assert.deepEqual(await persist.entries('pending'), []);
  second.client.dispose();
});
