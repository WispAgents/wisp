// Settings → Cloud → API keys on the mock (ADR-0058): `grid-client.keys`, `.keysCreate` and `.keysRevoke` answered
// as the grid answers them through wisp (plugins/grid-client/api/bindings/grid-client/{Keys,ApiKey,NewKey}.ts). The
// pane shows when the cloud row is signed in: POST /__mock/reset?seed=cloud (or MOCK_CLOUD=1). A made key is
// answered once and kept nowhere here either: the list holds its hint only.

import type { EntityOp } from '@wisp/protocol';

type Fn = (args: unknown) => { ops: EntityOp[]; result?: unknown };

interface Key {
  id: string;
  name: string;
  hint: string;
  createdAt: number;
  lastUsedAt: number | null;
}

const MIN = 60_000;
const ADDRESS = 'https://ada.trywisp.dev';

const seeded = (now: number): Key[] => [
  { id: 'a1b2c3d4e5f6', name: 'Deploy script', hint: 'Zq9x', createdAt: now - 3 * 24 * 60 * MIN, lastUsedAt: now - 12 * MIN },
  { id: '0f1e2d3c4b5a', name: 'GitHub Action', hint: 'p4Lm', createdAt: now - 9 * 24 * 60 * MIN, lastUsedAt: null },
];

let keys: Key[] | undefined;
const live = (): Key[] => (keys ??= seeded(Date.now()));

const refuse = (code: string, message: string): never => {
  throw { code, message };
};

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
const secret = (): string => `wgk_${Array.from({ length: 43 }, () => B64[Math.floor(Math.random() * 64)]).join('')}`;
const hex = (): string => Array.from({ length: 12 }, () => '0123456789abcdef'[Math.floor(Math.random() * 16)]).join('');

/** The mock starts over (POST /__mock/reset): the two seeded keys again. */
export function resetKeys(): void {
  keys = undefined;
}

export const KEY_COMMANDS: Record<string, Fn> = {
  // My shares, under API keys on the same pane: nothing shared yet
  'grid-client.shares': () => ({ ops: [], result: [] }),
  'grid-client.keys': () => ({ ops: [], result: { keys: live(), address: ADDRESS } }),
  'grid-client.keysCreate': (a) => {
    const name = String((a as { name?: unknown } | null)?.name ?? '').trim();
    if (!name) refuse('invalid', 'name: give the key a name');
    if ([...name].length > 60) refuse('invalid', 'name: keep a key’s name to 60 characters');
    if (live().length >= 20) refuse('invalid', 'You have 20 API keys, the most a grid keeps. Revoke one first.');
    const key = secret();
    const k: Key = { id: hex(), name, hint: key.slice(-4), createdAt: Date.now(), lastUsedAt: null };
    keys = [k, ...live()];
    return { ops: [], result: { ...k, key } };
  },
  'grid-client.keysRevoke': (a) => {
    const id = String((a as { id?: unknown } | null)?.id ?? '');
    if (!live().some((k) => k.id === id)) refuse('not_found', 'No such key.');
    keys = live().filter((k) => k.id !== id);
    return { ops: [], result: { ok: true } };
  },
};
