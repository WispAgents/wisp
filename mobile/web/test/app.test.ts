// ADR-0065 step 8: the app's start and its computers. It opens on Computers (open question 3's default: v1's), sets its
// platform's look on the root, and launching another computer runs the shell there while every computer keeps its
// own connection.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createClient, createScope, memoryHistory, peerId, type Json } from '@wisp/client';
import { applyPlatformLook } from '@wisp/shell/platform';
import { COMPUTERS_VIEW } from '@wisp/shell/pure';
import { bootPhone, phonePlatform, type PhoneWindow } from '../src/boot.ts';
import { computerRows, stateOf } from '../src/computers.ts';
import { createLauncher, LAUNCHED, startHome } from '../src/launch.ts';
import { followComputers, peerMap, phonePeers, PHONE } from '../src/peers.ts';
import { fakeBridge, fakeServer, fakeTimers, memoryStorage, settle, ssh, grid, welcome } from './helpers.ts';

/** The WebView's window as bootPhone and the platform read it; `kind` is what Capacitor says it runs on. */
function phoneWindow(kind: 'android' | 'ios', storage = memoryStorage()): PhoneWindow & { root: Map<string, string> } {
  const root = new Map<string, string>();
  const el = { setAttribute: (k: string, v: string) => void root.set(k, v), removeAttribute: (k: string) => void root.delete(k), getAttribute: (k: string) => root.get(k) ?? null };
  const media = (q: string): MediaQueryList => ({ matches: q.includes('pointer: coarse'), media: q, addEventListener: () => {}, removeEventListener: () => {} }) as unknown as MediaQueryList;
  return {
    root,
    navigator: { platform: kind === 'ios' ? 'iPhone' : 'Linux armv8l', userAgent: kind === 'ios' ? 'iPhone' : 'Android' } as Navigator,
    document: { documentElement: el, visibilityState: 'visible', addEventListener: () => {}, removeEventListener: () => {} } as unknown as Document,
    matchMedia: media,
    addEventListener: () => {},
    removeEventListener: () => {},
    open: () => null,
    innerWidth: 390,
    innerHeight: 844,
    location: { reload: () => {}, origin: 'capacitor://localhost', pathname: '/', search: '' },
    history: { pushState: () => {}, replaceState: () => {} },
    localStorage: storage,
    Capacitor: { isNativePlatform: () => true, getPlatform: () => kind },
  } as unknown as PhoneWindow & { root: Map<string, string> };
}

test('the_app_opens_on_computers: never inside a task, whatever the last place was', async () => {
  const bridge = fakeBridge([ssh('a', { name: 'Studio' })]);
  bridge.answers = [new Error('offline')]; // no network: the start doesn't wait on one
  const clock = fakeTimers();
  const phone = await bootPhone({
    build: 'test',
    window: phoneWindow('android'),
    bridge,
    client: { history: memoryHistory('/task/s1'), timers: clock },
    createShell: (client) => {
      client.plugin('phone').registerView({ id: COMPUTERS_VIEW, slot: 'main', parse: () => null as Json });
    },
  });
  assert.deepEqual(phone.client.nav.nav().main, { kind: 'view', id: COMPUTERS_VIEW, params: null });
  phone.client.nav.navigate({ t: 'back' });
  assert.deepEqual(phone.client.nav.nav().main, { kind: 'home' }, 'Back goes to Tasks, one tap away');
  phone.client.dispose();
});

test('the_phone_sets_its_look_on_the_root: Material 3 on Android, HIG on iOS', () => {
  for (const kind of ['android', 'ios'] as const) {
    const w = phoneWindow(kind);
    const platform = phonePlatform(w, fakeBridge());
    assert.equal(platform.kind, kind);
    assert.equal(platform.phone, true);
    assert.equal(applyPlatformLook(w.document.documentElement, platform), kind);
    assert.equal(w.root.get('data-platform'), kind);
  }
});

test('the shell starts on the computer launched last, else the first saved one, else the phone', () => {
  const list = [ssh('a', { peer: 'studio' }), grid('cloud')];
  assert.equal(startHome(list, memoryStorage()), 'studio');
  assert.equal(startHome(list, memoryStorage([[LAUNCHED, 'cloud']])), 'grid-cloud');
  assert.equal(startHome(list, memoryStorage([[LAUNCHED, 'gone']])), 'studio', 'a forgotten one: the first');
  assert.equal(startHome([], memoryStorage()), PHONE);
});

async function twoComputers(computers = [ssh('a', { name: 'Studio', peer: 'studio' }), ssh('b', { name: 'Laptop', peer: 'laptop' })]) {
  const clock = fakeTimers();
  const bridge = fakeBridge(computers);
  bridge.answers = [{ kind: 'ssh', port: 5001, token: 't', name: 'x' }];
  const srv = fakeServer();
  const storage = memoryStorage();
  const known = await bridge.list();
  const client = createClient({ clientId: 'phone-1', build: 'test', home: startHome(known, storage), width: 390, timers: clock, history: memoryHistory('/') });
  await client.start(await phonePeers(bridge, known, { fetch: srv.fetch }));
  const scope = createScope('follow', clock);
  followComputers(client, bridge, scope, known, { fetch: srv.fetch });
  for (const p of client.peers.list()()) client.peers.watch(p, 'list', client.scope);
  await settle(clock);
  srv.streams.forEach((s, i) => s.frame(null, welcome(i === 0 ? 'studio' : 'laptop')));
  await settle(clock);
  return { clock, bridge, srv, storage, client, scope };
}

test('switching_computers_keeps_each_peers_connection', async () => {
  const { clock, bridge, srv, storage, client, scope } = await twoComputers();
  const switched: string[] = [];
  const launcher = createLauncher({ client, storage, scope, onSwitch: (p) => switched.push(p) });
  const studio = peerId('studio');
  const laptop = peerId('laptop');
  assert.equal(client.home(), studio);
  const streams = srv.streams.length;
  const connects = bridge.connects.length;
  client.nav.navigate({ t: 'openView', id: COMPUTERS_VIEW as never, params: null });

  assert.equal(launcher.launch({ id: 'b', peer: laptop }), true);
  assert.equal(client.home(), laptop, 'the shell runs on Laptop');
  assert.deepEqual(switched, [laptop], 'the app runs its shell again, once');
  assert.deepEqual(client.nav.nav().main, { kind: 'home' }, 'and shows its tasks');
  assert.equal(storage.getItem(LAUNCHED), 'b', 'the next start runs on it again');
  await settle(clock);
  assert.equal(srv.streams.length, streams, 'no stream reopened');
  assert.equal(bridge.connects.length, connects, 'no tunnel asked again');
  assert.equal(client.peers.status(studio)().kind, 'online', 'Studio stays connected');
  assert.equal(client.peers.status(laptop)().kind, 'online');

  assert.equal(launcher.launch({ id: 'a', peer: studio }), true);
  assert.equal(launcher.launch({ id: 'a', peer: studio }), true);
  assert.deepEqual(switched, [laptop, studio], 'launching the one already running doesn’t run the shell again');
  assert.equal(launcher.launch({ id: 'x', peer: peerId('nowhere') }), false, 'not one of the saved computers');
  assert.equal(client.home(), studio);
  await settle(clock);
  assert.equal(srv.streams.length, streams);
  client.dispose();
  scope.dispose();
});

test('a forgotten launched computer hands the shell to the first one left', async () => {
  const { clock, bridge, storage, client, scope } = await twoComputers();
  const switched: string[] = [];
  const launcher = createLauncher({ client, storage, scope, onSwitch: (p) => switched.push(p) });
  launcher.launch({ id: 'b', peer: peerId('laptop') });
  await bridge.forget('b');
  await settle(clock);
  assert.equal(client.home(), 'studio');
  await bridge.forget('a');
  await settle(clock);
  assert.equal(client.home(), PHONE, 'no computer left: the phone, until one is added');
  await bridge.save(grid('cloud', { name: 'wisp Cloud' }));
  await settle(clock);
  assert.equal(client.home(), 'grid-cloud', 'the first one added runs the shell');
  assert.deepEqual(switched, ['laptop', 'studio', PHONE, 'grid-cloud']);
  client.dispose();
  scope.dispose();
});

test('the_launched_computer_follows_its_rekey: the first welcome names the wisp, the shell moves with it once its peer is there', async () => {
  const clock = fakeTimers();
  const bridge = fakeBridge([ssh('a', { name: 'Studio' }), ssh('b', { name: 'Laptop' })]);
  bridge.answers = [{ kind: 'ssh', port: 5001, token: 't', name: 'x' }];
  const srv = fakeServer();
  const storage = memoryStorage([[LAUNCHED, 'b']]);
  const known = await bridge.list();
  const client = createClient({ clientId: 'phone-1', build: 'test', home: startHome(known, storage), width: 390, timers: clock, history: memoryHistory('/') });
  const scope = createScope('follow', clock);
  const seen: [string, boolean][] = [];
  createLauncher({ client, storage, scope, onSwitch: (p) => seen.push([p, client.peers.list()().includes(p)]) });
  await client.start(await phonePeers(bridge, known, { fetch: srv.fetch }));
  followComputers(client, bridge, scope, known, { fetch: srv.fetch });
  assert.equal(client.home(), 'ssh-b', 'before any welcome: the derived id of the one launched last');
  for (const p of client.peers.list()()) client.peers.watch(p, 'list', client.scope);
  await settle(clock);
  srv.streams.forEach((s, i) => s.frame(null, welcome(i === 0 ? 'studio' : 'laptop')));
  await settle(clock);
  await settle(clock);
  assert.deepEqual(client.peers.list()().slice().sort(), ['laptop', 'studio'], 'both re-keyed');
  assert.equal(client.home(), 'laptop', 'the launched computer, not another one');
  assert.deepEqual(seen, [['laptop', true]], 'the shell ran again once, on a peer that is there');
  client.dispose();
  scope.dispose();
});

test('the Computers rows: how each is reached, its state as a word, the launched one marked', () => {
  const list = [ssh('a', { name: 'Studio', peer: 'studio' }), grid('cloud', { name: 'wisp Cloud' }), grid('acct.c1', { name: 'Laptop', computer: 'c1' })];
  const peers = peerMap(list);
  const status = (p: string) => (p === 'studio' ? { kind: 'online' as const } : p === 'grid-cloud' ? { kind: 'asleep' as const } : { kind: 'unreachable' as const, reason: 'x' });
  const rows = computerRows(list, peers, (p) => status(p), peerId('studio'));
  assert.deepEqual(
    rows.map((r) => [r.name, r.via, r.state, r.current]),
    [
      ['Studio', 'ssh', 'online', true],
      ['wisp Cloud', 'cloud', 'asleep', false],
      ['Laptop', 'grid', 'offline', false],
    ],
  );
  assert.equal(stateOf({ kind: 'waking' }), 'connecting');
  assert.equal(stateOf({ kind: 'incompatible', reason: 'old', update: 'peer' }), 'update');
  assert.equal(stateOf(undefined), 'offline');
});
