// One contract for every platform's WispSsh (ADR-0065 §1): the recorded Android answers
// (fixtures/android-bridge.json, also replayed against Computers.java by ComputersTest), and the
// bundle over them never putting a token in web storage. iOS's recording joins this suite (step 10).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { createClient, indexedDBPersist, type SessionId } from '@wisp/client';
import { capacitorBridge, hostKeyRefused, type NewComputer } from '../src/bridge.ts';
import { migrateV1 } from '../src/migrate.ts';
import { phonePeers, PHONE } from '../src/peers.ts';
import { androidNative } from './android-native.ts';
import { fakeIndexedDB, fakeServer, fakeTimers, memoryStorage, settle, welcome } from './helpers.ts';

type Step = { method: string; args: Record<string, unknown>; answer: unknown };
const recording = JSON.parse(readFileSync(new URL('./fixtures/android-bridge.json', import.meta.url), 'utf8')) as { steps: Step[] };

test('the Android fake answers exactly as the recording (and so as Computers.java)', async () => {
  const n = androidNative();
  for (const s of recording.steps) {
    assert.deepEqual(await n.cap.nativePromise?.('WispSsh', s.method, s.args), s.answer, `${s.method} ${JSON.stringify(s.args)}`);
  }
});

test('over the recording, the bridge never hands the page a token outside connect', async () => {
  const n = androidNative();
  const b = capacitorBridge(n.cap);
  const [g, s] = recording.steps.map((x) => x.args['computer']).filter(Boolean) as NewComputer[];
  if (!g || !s) throw new Error('the recording lost its saves');
  await b.save(g);
  await b.save(s);
  const listed = await b.list();
  assert.ok(!JSON.stringify(listed).includes('wlt_'), 'list() has no token');
  assert.ok(!JSON.stringify(listed).includes('hunter2'), 'nor a password');
  const studio = listed.find((c) => c.id === 's1');
  assert.equal(studio?.kind === 'ssh' ? studio.fingerprint : undefined, 'SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU', 'the pin is kept');
  assert.deepEqual(await b.connect('g1'), { kind: 'grid', base: 'https://mo.grid.test', token: 'wlt_dev_7Q2x', name: 'wisp Cloud' });
  assert.equal(await b.setStayConnected(true), true);
});

test('a refused host key reads back as the fingerprint Trust the new key needs', () => {
  assert.deepEqual(hostKeyRefused('This computer’s key changed: it is now SHA256:abc.'), { fingerprint: 'SHA256:abc', pinned: false });
  assert.deepEqual(hostKeyRefused('@@wisp fail ssh This computer’s key isn’t the one its QR code showed: it is now SHA256:def.'), { fingerprint: 'SHA256:def', pinned: true });
  assert.equal(hostKeyRefused('Authentication failed: the machine refused this phone’s key.'), undefined);
});

// ADR-0065 edge case: any token in web storage after a sign-in: never. The bundle's own start
// (bootPhone's steps: v1's list moved in, the client on IndexedDB, every computer a peer) with a
// v1 list holding a device token, a fresh sign-in, a renewed token and an SSH computer's
// server.json token; then nothing in localStorage or IndexedDB holds any of them.
test('tokens_never_land_in_web_storage', async () => {
  const secrets = ['wlt_v1_old_device', 'wlt_signin_new', 'wlt_renewed', 'wlt_ssh_server_json'];
  const n = androidNative();
  n.tunnels.set('s1', { port: 5011, token: 'wlt_ssh_server_json' });
  const bridge = capacitorBridge(n.cap);
  const ls = memoryStorage([['wisp.machines', JSON.stringify([{ id: 'g0', kind: 'grid', grid: 'https://grid.test', url: 'https://old.grid.test', token: 'wlt_v1_old_device' }, { id: 's1', target: 'me@studio' }])]]);
  const { idb, range, data } = fakeIndexedDB();
  const clock = fakeTimers();
  const srv = fakeServer();

  await migrateV1(ls, bridge);
  await bridge.save({ id: 'g1', kind: 'grid', name: 'wisp Cloud', url: 'https://mo.grid.test', token: 'wlt_signin_new' }); // the device flow's answer
  await bridge.save({ id: 'g0', token: 'wlt_renewed' });
  const client = createClient({ clientId: 'phone-1', build: 'test', home: PHONE, width: 390, timers: clock, persist: indexedDBPersist(idb, range) });
  const computers = await bridge.list();
  await client.start(await phonePeers(bridge, computers, { fetch: srv.fetch }));
  for (const p of client.peers.list()()) client.peers.watch(p, 'list', client.scope);
  await settle(clock);
  srv.streams.forEach((s, i) => {
    s.frame(null, welcome(`w-${i}`));
    s.frame(12, { t: 'snapshot', topic: 'list', seq: 12, rows: [{ e: 'session', id: `s${i}`, v: { title: 'hello' } }] });
  });
  await settle(clock);
  client.nav.navigate({ t: 'openTask', id: 'w-0:s0' as SessionId }); // the last screen is written too
  clock.advance(1_000); // past the rows' write debounce
  await client.flush();
  await settle(clock);

  // the tokens did reach the wire, as Bearer headers, so the test is not vacuous
  const sent = srv.requests.map((r) => r.headers['authorization'] ?? '').join(' ');
  for (const t of ['wlt_renewed', 'wlt_signin_new', 'wlt_ssh_server_json']) assert.ok(sent.includes(`Bearer ${t}`), `${t} was used`);
  assert.ok([...data.keys()].some((k) => k.startsWith('link')), 'the client wrote its peers’ links to IndexedDB');
  assert.ok([...data.keys()].some((k) => k.startsWith('rows')), 'and their rows');
  const stored = JSON.stringify([...ls.map]) + JSON.stringify([...data]);
  for (const t of secrets) assert.ok(!stored.includes(t), `${t} is in web storage`);
  assert.deepEqual([...n.vault.keys()].sort(), ['g0', 'g1'], 'the device tokens are in the Keystore');
  client.dispose();
});
