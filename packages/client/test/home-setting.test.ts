// homeSetting (doc25c): a server setting read without declaring it comes from this client's own wisp, never from
// whichever peer's `setting` row landed first, and follows setHome.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createClient } from '../src/client.ts';
import { createEntityStore } from '../src/entities.ts';
import { peerId } from '../src/ids.ts';
import { homeSetting } from '../src/settings.ts';
import { signal } from '../src/signals.ts';
import { fakeTimers } from './helpers.ts';

const HOME = peerId('laptop');
const REMOTE = peerId('box');

test('home_setting_reads_the_home_peers_row_and_follows_set_home', () => {
  const { reader, writer } = createEntityStore();
  // the other computer's row lands first: a bare list().find() took it
  writer.applyServer(REMOTE, [{ topic: 'config', e: 'setting', id: 'shell.projectOrder', op: 'upsert', v: { key: 'shell.projectOrder', value: ['b', 'a'] } }]);
  writer.applyServer(HOME, [{ topic: 'config', e: 'setting', id: 'shell.projectOrder', op: 'upsert', v: { key: 'shell.projectOrder', value: ['a', 'b'] } }]);
  assert.deepEqual(reader.list<{ key: string; value: unknown }>('setting')().find((r) => r.key === 'shell.projectOrder')?.value, ['b', 'a'], 'the bug: first row wins');
  const home = signal(HOME);
  const order = homeSetting<string[]>({ entities: reader, home }, 'shell.projectOrder');
  assert.deepEqual(order(), ['a', 'b']);
  home(REMOTE); // Client.setHome
  assert.deepEqual(order(), ['b', 'a']);
  assert.equal(homeSetting({ entities: reader, home }, 'shell.none')(), undefined);
});

test('a_plugin_clients_home_now_follows_set_home', () => {
  const client = createClient({ clientId: 'c1', build: 'test', home: HOME, width: 1280, timers: fakeTimers(1_700_000_000_000) });
  const p = client.plugin('wisp.list');
  assert.equal(p.homeNow(), HOME);
  client.setHome(REMOTE);
  assert.equal(p.homeNow(), REMOTE);
  assert.equal(p.home, REMOTE);
  client.dispose();
});
