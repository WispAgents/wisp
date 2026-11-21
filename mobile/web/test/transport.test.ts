import assert from 'node:assert/strict';
import { test } from 'node:test';
import { PeerDown, type Frame } from '@wisp/client';
import { bridgeTransport, gridAsleep } from '../src/transport.ts';
import { fakeBridge, fakeServer, welcome } from './helpers.ts';

const params = { client_id: 'c1', proto_min: '2', proto_max: '2' };

/** opens one connection and reads its first frame */
async function first(iter: AsyncIterable<Frame>): Promise<Frame | undefined> {
  for await (const f of iter) return f;
  return undefined;
}

test('each connect asks the bridge again: a tunnel back on a new port is followed', async () => {
  const bridge = fakeBridge();
  bridge.answers = [
    { kind: 'ssh', port: 5001, token: 'wlt_1', name: 'Studio' },
    { kind: 'ssh', port: 5002, token: 'wlt_1', name: 'Studio' },
  ];
  const srv = fakeServer();
  const t = bridgeTransport(bridge, { id: 'studio', kind: 'ssh' }, { fetch: srv.fetch });
  assert.equal(t.kind, 'ssh');
  assert.equal(t.wake, undefined, 'an SSH computer has nothing to wake');
  for (const port of [5001, 5002]) {
    const ctl = new AbortController();
    const frame = first(t.connect(params, ctl.signal));
    await new Promise((r) => setImmediate(r));
    srv.streams.at(-1)?.frame(null, welcome());
    assert.equal((await frame)?.messages[0]?.t, 'welcome');
    assert.match(srv.requests.at(-1)?.url ?? '', new RegExp(`^http://127\\.0\\.0\\.1:${port}/api/events\\?`));
    ctl.abort();
  }
  assert.deepEqual(bridge.connects, ['studio', 'studio']);
});

test('the token goes as a Bearer header on the stream and on every post, with the stream’s client id', async () => {
  const bridge = fakeBridge();
  bridge.answers = [{ kind: 'ssh', port: 5001, token: 'wlt_secret', name: 'Studio' }];
  const srv = fakeServer();
  const t = bridgeTransport(bridge, { id: 'studio', kind: 'ssh' }, { fetch: srv.fetch });
  const ctl = new AbortController();
  const frame = first(t.connect(params, ctl.signal));
  await new Promise((r) => setImmediate(r));
  srv.streams[0]?.frame(null, welcome());
  await frame;
  await t.post({ t: 'sub', topic: 'list' }, ctl.signal);
  const [get, post] = srv.requests;
  assert.equal(get?.headers['authorization'], 'Bearer wlt_secret');
  assert.equal(get?.headers['accept'], 'text/event-stream');
  assert.equal(post?.method, 'POST');
  assert.equal(post?.url, 'http://127.0.0.1:5001/api/events');
  assert.equal(post?.headers['authorization'], 'Bearer wlt_secret');
  assert.equal(post?.headers['x-wisp-client'], 'c1');
  assert.deepEqual(JSON.parse(post?.body ?? ''), { t: 'sub', topic: 'list' });
  ctl.abort();
});

test('a wisp with no token (older, or none written yet) gets no authorization header', async () => {
  const bridge = fakeBridge();
  bridge.answers = [{ kind: 'ssh', port: 4477, token: '', name: 'Studio' }];
  const srv = fakeServer();
  const ctl = new AbortController();
  const it = bridgeTransport(bridge, { id: 'studio', kind: 'ssh' }, { fetch: srv.fetch }).connect(params, ctl.signal);
  const frame = first(it);
  await new Promise((r) => setImmediate(r));
  srv.streams[0]?.frame(null, welcome());
  await frame;
  assert.equal('authorization' in (srv.requests[0]?.headers ?? {}), false);
  ctl.abort();
});

test('a bridge that can’t reach the computer is PeerDown unreachable, with its reason; nothing is fetched', async () => {
  const bridge = fakeBridge();
  bridge.answers = [new Error('Connection refused by studio:22.')];
  const srv = fakeServer();
  const t = bridgeTransport(bridge, { id: 'studio', kind: 'ssh' }, { fetch: srv.fetch });
  await assert.rejects(first(t.connect(params, new AbortController().signal)), (err: unknown) => {
    assert.ok(err instanceof PeerDown);
    assert.equal(err.status, 'unreachable');
    assert.equal(err.message, 'Connection refused by studio:22.');
    return true;
  });
  assert.equal(srv.requests.length, 0);
  await assert.rejects(t.post({ t: 'sub', topic: 'list' }, new AbortController().signal), PeerDown, 'no stream: nowhere to post');
});

test('a grid computer: its https address with the device token; asleep only for 503 + asleep, wake asks its health', async () => {
  const bridge = fakeBridge();
  bridge.answers = [{ kind: 'grid', base: 'https://mo.grid.test', token: 'dev-1', name: 'wisp Cloud' }];
  const srv = fakeServer();
  srv.reply = (r) => (r.url.includes('/api/events?') ? { status: 503, body: '{"asleep":true}' } : undefined);
  const t = bridgeTransport(bridge, { id: 'cloud', kind: 'grid' }, { fetch: srv.fetch });
  assert.equal(t.kind, 'grid');
  await assert.rejects(first(t.connect(params, new AbortController().signal)), (err: unknown) => err instanceof PeerDown && err.status === 'asleep');
  assert.match(srv.requests[0]?.url ?? '', /^https:\/\/mo\.grid\.test\/api\/events\?/);
  assert.equal(srv.requests[0]?.headers['authorization'], 'Bearer dev-1');
  srv.reply = (r) => (r.url.includes('/api/events?') ? { status: 503, body: '{"error":"It’s still waking up.","waking":true}' } : undefined);
  await assert.rejects(first(t.connect(params, new AbortController().signal)), (err: unknown) => err instanceof PeerDown && err.status === 'unreachable');
  await t.wake?.(new AbortController().signal);
  const health = srv.requests.at(-1);
  assert.equal(health?.url, 'https://mo.grid.test/api/health');
  assert.equal(health?.headers['authorization'], 'Bearer dev-1');
  srv.reply = () => ({ status: 401, body: '{"error":"Sign in again."}' });
  await assert.rejects(t.wake?.(new AbortController().signal) ?? Promise.resolve(), /Signed out of wisp Cloud/);
});

test('gridAsleep: only a 503 that says asleep', () => {
  assert.equal(gridAsleep(503, '{"asleep":true}'), true);
  assert.equal(gridAsleep(503, '{"waking":true}'), false);
  assert.equal(gridAsleep(503, 'Service Unavailable'), false);
  assert.equal(gridAsleep(502, '{"asleep":true}'), false);
});

test('an_older_wisp_says_update', async () => {
  const { createClient, peerId } = await import('@wisp/client');
  const { phonePeers, PHONE } = await import('../src/peers.ts');
  const { fakeTimers, settle, ssh } = await import('./helpers.ts');
  const clock = fakeTimers();
  const bridge = fakeBridge([ssh('a', { peer: 'studio-1f3a', name: 'Studio' })]);
  bridge.answers = [{ kind: 'ssh', port: 5001, token: 't', name: 'Studio' }];
  const srv = fakeServer();
  const client = createClient({ clientId: 'phone-1', build: 'test', home: PHONE, width: 390, timers: clock });
  await client.start(await phonePeers(bridge, undefined, { fetch: srv.fetch }));
  await settle(clock);
  // a wisp whose protocol window ends before the app's begins: it says so instead of half working
  srv.streams[0]?.frame(null, welcome('studio-1f3a', { proto_min: 1, proto_max: 1 }));
  await settle(clock);
  assert.deepEqual(client.peers.status(peerId('studio-1f3a'))(), { kind: 'incompatible', reason: 'Update wisp on Studio', update: 'peer', command: 'npm install -g wisp-agents@latest' });
  clock.advance(10 * 60_000);
  await settle(clock);
  assert.equal(srv.streams.length, 1, 'no retry loop against a wisp it cannot speak to');
  client.dispose();
});

test('a route call (an attached file’s POST /uploads) goes to the computer with its token, connected or not', async () => {
  const bridge = fakeBridge();
  bridge.answers = [{ kind: 'ssh', port: 5001, token: 'wlt_secret', name: 'Studio' }];
  const srv = fakeServer();
  srv.reply = (r) => (r.url.includes('/api/uploads') ? { status: 200, body: '{"id":"u1","name":"a.png","mime":"image/png","size":3}' } : undefined);
  const t = bridgeTransport(bridge, { id: 'studio', kind: 'ssh' }, { fetch: srv.fetch });
  assert.ok(t.request && t.events, 'the phone reaches plugin routes');
  const res = await t.request({ method: 'POST', path: '/uploads?name=a.png&type=image%2Fpng', body: 'abc', contentType: 'application/octet-stream' }, new AbortController().signal);
  assert.equal(res.status, 200);
  assert.equal(JSON.parse(res.text).id, 'u1');
  const up = srv.requests.at(-1);
  assert.equal(up?.method, 'POST');
  assert.equal(up?.url, 'http://127.0.0.1:5001/api/uploads?name=a.png&type=image%2Fpng');
  assert.equal(up?.headers['authorization'], 'Bearer wlt_secret');
  assert.equal(up?.headers['content-type'], 'application/octet-stream');
  assert.deepEqual(bridge.connects, ['studio']);
});
