// The Android side of WispSsh as the bundle sees it, kept in step with Computers.java by the recording
// in fixtures/android-bridge.json (bridge-contract.test.ts replays it here; ComputersTest replays it
// against the Java). Tokens go into `vault`, never into the list; connect answers them per call.

import type { CapacitorLike } from '../src/bridge.ts';

type Obj = Record<string, unknown>;

export interface AndroidNative {
  readonly cap: CapacitorLike;
  /** what the Keystore holds: device tokens by computer id */
  readonly vault: Map<string, string>;
  /** the ssh computers' answers to connect (the tunnel's port and server.json token) */
  readonly tunnels: Map<string, { port: number; token: string }>;
  readonly calls: [string, Obj][];
}

export function androidNative(): AndroidNative {
  let list: Obj[] = [];
  const vault = new Map<string, string>();
  const tunnels = new Map<string, { port: number; token: string }>();
  const calls: [string, Obj][] = [];
  const keep = (o: Obj): Obj => {
    const { token, password: _password, ...rest } = o;
    if (typeof token === 'string' && token) vault.set(String(o['id']), token);
    return rest;
  };
  const methods: Record<string, (a: Obj) => unknown> = {
    list: () => ({ computers: structuredClone(list) }),
    saveComputer: (a) => {
      const c = keep(structuredClone(a['computer'] as Obj));
      const i = list.findIndex((x) => x['id'] === c['id']);
      if (i < 0 && !c['kind']) throw new Error('a new computer needs a kind');
      if (i >= 0) list[i] = { ...list[i], ...c };
      else list.push(c);
      return structuredClone(list[i >= 0 ? i : list.length - 1]);
    },
    forgetComputer: (a) => {
      list = list.filter((x) => x['id'] !== a['id']);
      vault.delete(String(a['id']));
      return {};
    },
    connect: (a) => {
      const c = list.find((x) => x['id'] === a['id']);
      if (!c) throw new Error(`No saved computer ${String(a['id'])}.`);
      const base = { name: c['name'] ?? c['id'], ...(c['peer'] ? { peer: c['peer'] } : {}) };
      if (c['kind'] === 'grid') return { kind: 'grid', base: c['url'], token: vault.get(String(c['id'])) ?? '', ...base };
      const t = tunnels.get(String(c['id']));
      if (!t) throw new Error('Connection refused by studio:22.');
      return { kind: 'ssh', port: t.port, token: t.token, ...base };
    },
    disconnect: () => ({}),
    trustHostKey: () => ({}),
    setStayConnected: (a) => ({ on: a['on'] === true }),
  };
  const cap: CapacitorLike = {
    isNativePlatform: () => true,
    nativePromise: async (plugin, method, args = {}) => {
      if (plugin !== 'WispSsh') throw new Error(`${plugin} is not faked`);
      calls.push([method, structuredClone(args) as Obj]);
      const fn = methods[method];
      if (!fn) throw new Error(`WispSsh.${method} is not implemented`);
      return fn(args as Obj);
    },
    addListener: () => ({ remove: () => {} }),
  };
  return { cap, vault, tunnels, calls };
}
