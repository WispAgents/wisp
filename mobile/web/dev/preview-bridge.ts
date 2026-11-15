// The phone app in a desktop browser (`npm run dev` here): a stand-in for the native side with two saved computers,
// each reached over plain HTTP on the dev server's mock peers (apps/web/dev/vite-mock.ts with MOCK_COMPUTERS=1: this
// one at /api, Studio at /api/peer/studio). `?platform=ios` draws the iPhone app, anything else the Android one.
import type { Computer, ComputerPatch, NativeBridge, NewComputer, Reached } from '../src/bridge.ts';

export function previewLook(search: string): 'android' | 'ios' {
  return new URLSearchParams(search).get('platform') === 'ios' ? 'ios' : 'android';
}

export function previewBridge(origin: string): NativeBridge {
  let computers: Computer[] = [
    { id: 'mac', kind: 'grid', name: 'Mac mini', url: origin },
    { id: 'studio', kind: 'ssh', name: 'Studio', target: 'me@studio' },
  ];
  const listeners = { computers: new Set<() => void>() };
  const base: Record<string, string> = { mac: origin, studio: `${origin}/api/peer/studio` };
  return {
    list: async () => [...computers],
    async save(c: NewComputer | ComputerPatch) {
      const { token: _token, ...rest } = c as { token?: string } & Computer;
      const i = computers.findIndex((x) => x.id === c.id);
      computers = i >= 0 ? computers.map((x, j) => (j === i ? ({ ...x, ...rest } as Computer) : x)) : [...computers, rest as Computer];
      for (const fn of listeners.computers) fn();
    },
    async forget(id) {
      computers = computers.filter((c) => c.id !== id);
      for (const fn of listeners.computers) fn();
    },
    async connect(id): Promise<Reached> {
      const c = computers.find((x) => x.id === id);
      const at = base[id];
      if (!c || !at) throw new Error('no such computer');
      return { kind: 'grid', base: at, token: '', name: c.name };
    },
    disconnect: async () => {},
    trustHostKey: async () => {},
    setStayConnected: async (on) => on,
    leave: async () => {},
    onTunnel: () => () => {},
    onComputers: (fn) => {
      listeners.computers.add(fn);
      return () => listeners.computers.delete(fn);
    },
    onBack: () => () => {},
    onOpen: () => () => {},
  };
}
