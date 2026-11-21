// A stand-in wisp Grid for the phone app's preview (`npm run dev` here): the device flow (a code; the first poll still
// pending, the next allowed, as if you tapped Allow in the browser), and the account's computers. `?cloud=signed-in`
// starts signed in. With it, an in-memory sealed store (the Keystore / Keychain stand-in): the preview never writes a
// token to web storage either. Dev only (main.tsx imports it outside the apps).
import type { FetchLike, FetchResponse } from '@wisp/client';
import type { SecureStore } from '@wisp/shell/platform';
import { accountKey, ACCOUNTS, DEFAULT_GRID, tokenKey } from '../src/cloud.ts';

const TOKEN = 'wgd_preview';
const HANDLE = 'ada';

const answer = (status: number, body: unknown): FetchResponse => ({
  status,
  ok: status < 400,
  body: null,
  text: async () => JSON.stringify(body),
});

export function previewGrid(search: string, storage: Pick<Storage, 'setItem'>): { fetch: FetchLike; secure: SecureStore } {
  const sealed = new Map<string, string>();
  let polls = 0;
  if (new URLSearchParams(search).get('cloud') === 'signed-in') {
    storage.setItem(
      ACCOUNTS,
      JSON.stringify([
        {
          grid: DEFAULT_GRID,
          handle: HANDLE,
          name: 'Ada Lovelace',
          via: 'github',
        },
      ]),
    );
    sealed.set(tokenKey(accountKey(DEFAULT_GRID, HANDLE)), TOKEN);
  }
  const fetch: FetchLike = async (url, init) => {
    const path = new URL(url).pathname;
    if (path === '/api/device/code')
      return answer(200, {
        device_code: 'dc-preview',
        user_code: 'WISP-7Q4K',
        interval: 1,
        expires_in: 600,
        verification_uri: `${DEFAULT_GRID}/activate`,
      });
    if (path === '/api/device/token')
      return ++polls < 2
        ? answer(400, { error: 'authorization_pending' })
        : answer(200, {
            token: TOKEN,
            user: { handle: HANDLE, name: 'Ada Lovelace' },
            instance: {
              url: `https://${HANDLE}.grid.trywisp.dev`,
              name: 'Cloud',
              handle: HANDLE,
            },
          });
    const auth = init.headers['authorization'];
    if (path === '/api/computers') {
      if (auth !== `Bearer ${TOKEN}`) return answer(401, { error: 'Sign in first.' });
      return answer(200, [
        {
          id: 'c-laptop',
          name: 'Work laptop',
          label: `laptop--${HANDLE}`,
          url: `https://laptop--${HANDLE}.grid.trywisp.dev`,
          online: true,
        },
        {
          id: 'c-tower',
          name: 'Tower',
          label: `tower--${HANDLE}`,
          url: `https://tower--${HANDLE}.grid.trywisp.dev`,
          online: false,
        },
      ]);
    }
    if (path === '/api/device/revoke') return answer(200, { ok: true });
    return answer(404, { error: 'not found' });
  };
  const secure: SecureStore = {
    get: async (k) => sealed.get(k) ?? null,
    set: async (k, v) => (sealed.set(k, v), 'set'),
    remove: async (k) => void sealed.delete(k),
  };
  return { fetch, secure };
}
