// The phone launches another computer without a reload (Client.setHome): the list, made once per plugin client, took
// its project order and its Later watch from the first computer for good. Both follow the home now.
import { expect, test } from 'vitest';
import { createClient, peerId, signal, untracked, type PluginClient, type Scope } from '@wisp/client';
import { PROJECT_ORDER } from '@wisp/shell/pure';
import { modelOf } from '../src/state.ts';
import { LATER_TOPIC } from '../src/model/wire.ts';

const A = peerId('laptop');
const B = peerId('studio');

test('the_list_follows_set_home_its_project_order_and_the_later_watch_move_to_the_new_computer', () => {
  const client = createClient({ clientId: 'c1', build: 'test', home: A, width: 1280 });
  const real = client.plugin('wisp.shell-list');
  const home = signal(A);
  const watched: { peer: string; topic: string; owner: Scope | undefined }[] = [];
  const order = (peer: string, value: string[]) => ({ key: PROJECT_ORDER, value, peer });
  const projects = [
    { id: 'web', body: { path: '/w/web' } },
    { id: 'api', body: { path: '/w/api' } },
  ];
  // B's order arrives first: a first-row-wins read gave it while A was home
  const settings = [order(B, ['api', 'web']), order(A, ['web', 'api'])];
  const entities = {
    ...real.entities,
    list: (kind: string) => (kind === 'project' ? () => projects : kind === 'setting' ? () => settings : real.entities.list(kind)),
    row: (kind: string, key: string) => (kind === 'setting' ? () => settings.find((s) => `${s.peer}:${s.key}` === key) : real.entities.row(kind, key as never)),
  };
  const fake = {
    ...real,
    id: real.id,
    entities,
    homeNow: home,
    get home() {
      return untracked(home);
    },
    watch: (peer: string, topic: string, owner?: Scope) => void watched.push({ peer, topic, owner }),
  } as unknown as PluginClient;
  const m = modelOf(fake);
  expect(m.projects().map((p) => p.id)).toEqual(['web', 'api']);
  expect(watched.map((w) => [w.peer, w.topic])).toEqual([[A, LATER_TOPIC]]);
  const first = watched[0]?.owner;
  let ended = false;
  first?.add(() => (ended = true));

  home(B); // Client.setHome(B)
  expect(m.projects().map((p) => p.id)).toEqual(['api', 'web']);
  expect(watched.map((w) => [w.peer, w.topic])).toEqual([
    [A, LATER_TOPIC],
    [B, LATER_TOPIC],
  ]);
  expect(ended).toBe(true); // the first computer's watch ended with its scope
  client.dispose();
});
