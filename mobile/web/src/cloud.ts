// wisp Cloud on the phone (ADR-0065 §2.3-4, design/mobile.md §A.2 Computers, F-phone-5): signing in to a wisp Grid
// from the app and listing the account's computers. The phone has no wisp of its own, so it runs the grid's device
// flow itself, the same one the desktop's grid-client runs (RFC 8628, v1 mobile/www/app.js): POST /api/device/code,
// the system browser (Custom Tabs / ASWebAuthenticationSession) at /auth/<provider>/start returning to /activate with
// the code, then polling /api/device/token. The device token it gets goes to the device's sealed store only (the
// Keystore / Keychain: `SecureStore`, round 226's seam), never web storage; each computer is saved through the native
// bridge with it (`NewComputer.token`, sealed there too). What this file keeps in web storage is the account list
// (grid, handle, name, how it signed in): no secret. Network and time are injected (fetch, sleep), so it's tested
// without either.
import type { FetchLike } from '@wisp/client';
import type { SecureStore } from '@wisp/shell/platform';
import type { Computer, GridComputer, NewComputer } from './bridge.ts';

/** The hosted grid (wisp Cloud). Yours is behind *Use your own wisp Grid*. */
export const DEFAULT_GRID = 'https://grid.trywisp.dev';

export type Provider = 'github' | 'google' | 'apple';

/** The one-tap ways in: GitHub and Google. Apple joins on iOS once the grid has an Apple sign-in (ADR-0065 open
 *  question 1: before the iOS app ships); today the grid's providers are github, google and oidc (apps/grid auth.rs), so
 *  an Apple button would open "That way of signing in isn't set up". */
export function providersFor(_look: 'android' | 'ios'): readonly Provider[] {
  return ['github', 'google'];
}

const LOCAL = /(^|\.)localhost$|^127\.0\.0\.1$/i;

export type GridAddressError = 'empty' | 'insecure' | 'invalid';

/** `wisp.example.com` → `https://wisp.example.com`; plain http only for a grid on this phone's loopback (the sign-in
 *  goes over it). */
export function gridUrl(raw: string): string | { readonly error: GridAddressError } {
  let s = raw.trim();
  if (!s) return { error: 'empty' };
  if (!/^https?:\/\//i.test(s)) s = `${LOCAL.test(s.split(/[:/]/)[0] ?? '') ? 'http' : 'https'}://${s}`;
  let u: URL;
  try {
    u = new URL(s);
  } catch {
    return { error: 'invalid' };
  }
  if (!u.hostname.includes('.') && !LOCAL.test(u.hostname)) return { error: 'invalid' };
  if (u.protocol === 'http:' && !LOCAL.test(u.hostname)) return { error: 'insecure' };
  return u.origin;
}

/** `grid.trywisp.dev` for `https://grid.trywisp.dev`. */
export const hostOf = (url: string): string => url.replace(/^https?:\/\//, '').replace(/\/$/, '');

/** One signed-in account: one card on the Computers screen (ADR-0061: several, side by side). */
export interface CloudAccount {
  /** `<grid>#<handle>` */
  readonly key: string;
  readonly grid: string;
  readonly handle: string;
  /** the person's name on the grid (or the handle) */
  readonly name: string;
  readonly via?: Provider;
  /** the grid answered 401: its token was revoked; the card keeps its computers and asks to sign in again */
  readonly signedOut?: boolean;
}

export const accountKey = (grid: string, handle: string): string => `${grid}#${handle}`;
/** where an account's device token is sealed (SecureStore) */
export const tokenKey = (key: string): string => `wisp.cloud.token.${key}`;

/** What /api/device/code answered: the code to show, where to allow it, how often to ask. */
export interface Flow {
  readonly grid: string;
  readonly deviceCode: string;
  readonly userCode: string;
  readonly verify: string;
  readonly interval: number;
  readonly expiresAt: number;
}

/** What /api/device/token handed over once allowed. */
export interface Granted {
  readonly token: string;
  readonly handle: string;
  readonly name: string;
  /** the account's own wisp (its cloud box) */
  readonly instance: { readonly url: string; readonly name: string };
}

/** One of the account's computers, as GET /api/computers lists it. */
export interface RemoteComputer {
  readonly id: string;
  readonly name: string;
  readonly url: string;
  readonly online: boolean;
}

export type CloudErrorCode = 'unreachable' | 'notGrid' | 'denied' | 'expired' | 'cancelled' | 'noVault' | 'signedOut';

export class CloudError extends Error {
  readonly code: CloudErrorCode;
  constructor(code: CloudErrorCode, message: string = code) {
    super(message);
    this.code = code;
  }
}

const rec = (v: unknown): Record<string, unknown> => (v && typeof v === 'object' ? (v as Record<string, unknown>) : {});
const str = (v: unknown): string => (typeof v === 'string' ? v : '');
const json = (token?: string): Record<string, string> => ({
  'content-type': 'application/json',
  accept: 'application/json',
  ...(token && { authorization: `Bearer ${token}` }),
});

async function call(
  fetch: FetchLike,
  url: string,
  init: {
    method: string;
    body?: unknown;
    token?: string;
    signal?: AbortSignal;
  },
): Promise<{ status: number; body: Record<string, unknown> | unknown[] }> {
  let res;
  try {
    res = await fetch(url, {
      method: init.method,
      headers: json(init.token),
      ...(init.body !== undefined && { body: JSON.stringify(init.body) }),
      signal: init.signal ?? new AbortController().signal,
    });
  } catch (err) {
    if (init.signal?.aborted) throw new CloudError('cancelled');
    throw new CloudError('unreachable', err instanceof Error ? err.message : String(err));
  }
  let body: unknown = {};
  try {
    body = JSON.parse(await res.text());
  } catch {
    body = {};
  }
  return { status: res.status, body: Array.isArray(body) ? body : rec(body) };
}

/** Asks the grid for a code. `now` stamps when it expires. */
export async function startFlow(
  fetch: FetchLike,
  grid: string,
  o: {
    readonly name: string;
    readonly now: number;
    readonly signal?: AbortSignal;
  },
): Promise<Flow> {
  const r = await call(fetch, `${grid}/api/device/code`, {
    method: 'POST',
    body: { name: o.name },
    ...(o.signal && { signal: o.signal }),
  });
  const b = rec(r.body);
  const deviceCode = str(b['device_code']);
  if (r.status >= 400 || !deviceCode) throw new CloudError('notGrid');
  const interval = Math.max(1, Number(b['interval']) || 5);
  const expires = Number(b['expires_in']) || 600;
  return {
    grid,
    deviceCode,
    userCode: str(b['user_code']),
    verify: str(b['verification_uri_complete']) || str(b['verification_uri']),
    interval,
    expiresAt: o.now + expires * 1000,
  };
}

/** Where the system browser goes: the provider's sign-in, back to /activate with the code (app=… so the grid's page
 *  links back to the app, not to the account page). No provider: the grid's own /activate page. */
export function signInUrl(f: Pick<Flow, 'grid' | 'userCode' | 'verify'>, via: Provider | undefined, app: 'android' | 'ios'): string {
  const activate = `/activate?code=${encodeURIComponent(f.userCode)}&app=${app}`;
  if (via) return `${f.grid}/auth/${via}/start?return=${encodeURIComponent(activate)}`;
  const base = f.verify || `${f.grid}${activate}`;
  return base.includes('app=') ? base : `${base}${base.includes('?') ? '&' : '?'}app=${app}`;
}

export interface PollDeps {
  readonly fetch: FetchLike;
  /** waits `ms` (rejects when the signal aborts) */
  readonly sleep: (ms: number, signal: AbortSignal) => Promise<void>;
  readonly now: () => number;
}

/**
 * Polls until the code is allowed, declined or expired (RFC 8628 §3.5): the grid's interval, +5 s per `slow_down`; a
 * network blip, a 5xx or a 429 keeps asking. `nudge` (the app back from the browser) asks at once instead of waiting.
 */
export async function pollFlow(f: Flow, d: PollDeps, signal: AbortSignal, nudge?: (wake: () => void) => () => void): Promise<Granted> {
  let interval = f.interval;
  for (;;) {
    await wait(d, interval * 1000, signal, nudge);
    if (signal.aborted) throw new CloudError('cancelled');
    if (d.now() > f.expiresAt) throw new CloudError('expired');
    let r;
    try {
      r = await call(d.fetch, `${f.grid}/api/device/token`, {
        method: 'POST',
        body: { device_code: f.deviceCode },
        signal,
      });
    } catch (err) {
      if (err instanceof CloudError && err.code === 'cancelled') throw err;
      continue; // offline for a moment: ask again
    }
    const b = rec(r.body);
    const e = str(b['error']);
    const token = str(b['token']);
    if (e === 'slow_down') interval += 5;
    if (e === 'authorization_pending' || e === 'slow_down' || r.status === 429 || r.status >= 500 || (!e && !token)) continue;
    if (e === 'access_denied') throw new CloudError('denied');
    // invalid_grant: the code was used already or the grid forgot it (apps/grid accounts.rs poll): start again
    if (e === 'expired_token' || e === 'invalid_grant') throw new CloudError('expired');
    if (e) throw new CloudError('notGrid', str(b['error_description']) || e);
    const user = rec(b['user']);
    const inst = rec(b['instance']);
    const handle = str(user['handle']) || str(inst['handle']);
    return {
      token,
      handle,
      name: str(user['name']) || handle,
      instance: { url: str(inst['url']), name: str(inst['name']) || 'Cloud' },
    };
  }
}

/** AbortSignal.any for WebViews older than it (Chrome 116, Safari 17.4), with the way to take its listeners off (one
 *  per poll on the flow's signal would pile up over a ten-minute flow). */
function anySignal(a: AbortSignal, b: AbortSignal): { readonly signal: AbortSignal; readonly off: () => void } {
  const out = new AbortController();
  const stop = (): void => out.abort();
  if (a.aborted || b.aborted) stop();
  a.addEventListener('abort', stop, { once: true });
  b.addEventListener('abort', stop, { once: true });
  return {
    signal: out.signal,
    off: () => {
      a.removeEventListener('abort', stop);
      b.removeEventListener('abort', stop);
    },
  };
}

function wait(d: PollDeps, ms: number, signal: AbortSignal, nudge?: (wake: () => void) => () => void): Promise<void> {
  if (!nudge) return d.sleep(ms, signal).catch(() => undefined);
  const early = new AbortController();
  const stop = nudge(() => early.abort());
  const both = anySignal(signal, early.signal);
  return d
    .sleep(ms, both.signal)
    .catch(() => undefined)
    .finally(() => {
      stop();
      both.off();
    });
}

/** GET /api/computers with the account's token; 'signedOut' on a 401 (a revoked token). An older grid answered
 *  `{ computers: [...] }`, the v2 grid the array. */
export async function listComputers(fetch: FetchLike, grid: string, token: string, signal?: AbortSignal): Promise<readonly RemoteComputer[] | 'signedOut'> {
  const r = await call(fetch, `${grid}/api/computers`, {
    method: 'GET',
    token,
    ...(signal && { signal }),
  });
  if (r.status === 401 || r.status === 403) return 'signedOut';
  if (r.status >= 400) throw new CloudError('unreachable', `HTTP ${r.status}`);
  const raw = Array.isArray(r.body) ? r.body : rec(r.body)['computers'];
  return (Array.isArray(raw) ? raw : []).flatMap((x) => {
    const o = rec(x);
    const id = str(o['id']);
    const url = str(o['url']);
    return id && url ? [{ id, name: str(o['name']) || id, url, online: o['online'] === true }] : [];
  });
}

/**
 * Signing out tells the grid (`POST /api/device/revoke` with the token itself, ADR-0078), so the token stops working
 * there too, not only on this phone. True once the grid won't take it any more: `{ ok: true }` (revoked now or
 * before), 401/403 (it doesn't know it), 404 (a grid from before the route: it can't be told; the account page removes
 * the device there). False otherwise (offline, 5xx, 429, a portal's page, no answer within `ms`): tell it again later.
 */
export async function tellRevoked(fetch: FetchLike, grid: string, token: string, sleep?: PollDeps['sleep'], ms = 10_000): Promise<boolean> {
  const ctl = new AbortController();
  const timer = new AbortController();
  if (sleep) void sleep(ms, timer.signal).then(() => ctl.abort(), () => undefined);
  try {
    const r = await call(fetch, `${grid}/api/device/revoke`, { method: 'POST', body: {}, token, signal: ctl.signal });
    // a 2xx only from the grid itself (a captive portal's 200 page isn't), the grid's own refusals are final, the rest
    // (5xx, 429, a proxy's 3xx or 421) is asked again later
    if (r.status >= 200 && r.status < 300) return rec(r.body)['ok'] === true;
    return r.status === 401 || r.status === 403 || r.status === 404;
  } catch {
    return false;
  } finally {
    timer.abort();
  }
}

/** A sign-out the grid couldn't be told of yet: its grid, and where its token is sealed meanwhile (SecureStore). */
export interface PendingRevoke {
  readonly grid: string;
  readonly slot: string;
}

/** Where the sign-outs to tell the grid of are listed: web storage, with no token in it (the token: `slot`). */
export const REVOKES = 'wisp.phone.cloud.revoke';
/** where a pending sign-out's token is sealed */
export const revokeSlot = (n: string): string => `wisp.cloud.revoke.${n}`;

export function loadRevokes(storage: KV): PendingRevoke[] {
  try {
    const raw: unknown = JSON.parse(storage.getItem(REVOKES) ?? '[]');
    return (Array.isArray(raw) ? raw : []).flatMap((x) => {
      const o = rec(x);
      const grid = str(o['grid']);
      const slot = str(o['slot']);
      return grid && slot.startsWith(revokeSlot('')) ? [{ grid, slot }] : [];
    });
  } catch {
    return [];
  }
}

export function saveRevokes(storage: KV, list: readonly PendingRevoke[]): void {
  storage.setItem(REVOKES, JSON.stringify(list.map((p) => ({ grid: p.grid, slot: p.slot }))));
}

/** The saved id of an account's computer: stable, so listing again updates it in place. The cloud box is `box`. */
export const savedId = (a: Pick<CloudAccount, 'key'>, computer: string): string => `cloud:${a.key}:${computer}`;

/**
 * What to save for an account: its cloud box first, then its computers on the grid, each with the account's token
 * (the native side seals it). Ones already saved and unchanged are left out; changed ones (a rename, a new address)
 * are saved again in place. `fresh` (a new sign-in: a new token) saves every one again, or the computers already saved
 * would keep the old, revoked token.
 */
export function toSave(
  a: CloudAccount,
  token: string,
  box: { readonly url: string; readonly name: string } | undefined,
  list: readonly RemoteComputer[],
  saved: readonly Computer[],
  fresh = false,
): NewComputer[] {
  const want: (GridComputer & { token: string })[] = [];
  if (box?.url)
    want.push({
      id: savedId(a, 'box'),
      kind: 'grid',
      name: box.name,
      url: box.url,
      grid: a.grid,
      account: a.key,
      token,
    });
  for (const c of list)
    want.push({
      id: savedId(a, c.id),
      kind: 'grid',
      name: c.name,
      url: c.url,
      grid: a.grid,
      computer: c.id,
      account: a.key,
      token,
    });
  return want.filter((w) => {
    if (fresh) return true;
    const had = saved.find((s) => s.id === w.id);
    return !had || had.kind !== 'grid' || had.name !== w.name || had.url !== w.url;
  });
}

/** The saved computers that are this account's (its card's rows), the box first. */
export const accountComputers = (a: Pick<CloudAccount, 'key'>, saved: readonly Computer[]): Computer[] =>
  saved.filter((c) => c.kind === 'grid' && c.account === a.key).sort((x, y) => Number(!!(x as GridComputer).computer) - Number(!!(y as GridComputer).computer));

/** The account's computers the grid no longer lists (removed there): forgotten here too. The box stays. */
export const gone = (a: Pick<CloudAccount, 'key'>, list: readonly RemoteComputer[], saved: readonly Computer[]): string[] =>
  accountComputers(a, saved)
    .filter((c) => c.kind === 'grid' && c.computer && !list.some((r) => savedId(a, r.id) === c.id))
    .map((c) => c.id);

/** Where the account list lives: web storage, with no token in it (tokens: SecureStore). */
export const ACCOUNTS = 'wisp.phone.cloud';

type KV = {
  getItem(k: string): string | null;
  setItem(k: string, v: string): void;
};

export function loadAccounts(storage: KV): CloudAccount[] {
  try {
    const raw: unknown = JSON.parse(storage.getItem(ACCOUNTS) ?? '[]');
    return (Array.isArray(raw) ? raw : []).flatMap((x) => {
      const o = rec(x);
      const grid = str(o['grid']);
      const handle = str(o['handle']);
      if (!grid || !handle) return [];
      const via = str(o['via']);
      return [
        {
          key: accountKey(grid, handle),
          grid,
          handle,
          name: str(o['name']) || handle,
          ...((via === 'github' || via === 'google' || via === 'apple') && {
            via,
          }),
          ...(o['signedOut'] === true && { signedOut: true }),
        },
      ];
    });
  } catch {
    return [];
  }
}

export function saveAccounts(storage: KV, list: readonly CloudAccount[]): void {
  // only these fields, whatever an account object carries: never a token
  storage.setItem(
    ACCOUNTS,
    JSON.stringify(
      list.map((a) => ({
        grid: a.grid,
        handle: a.handle,
        name: a.name,
        ...(a.via && { via: a.via }),
        ...(a.signedOut && { signedOut: true }),
      })),
    ),
  );
}

/** Seals the token; a phone without a sealed store refuses (never a fallback to web storage). */
export async function keepToken(secure: SecureStore | undefined, key: string, token: string): Promise<void> {
  const r = secure ? await secure.set(tokenKey(key), token) : 'unsupported';
  if (r !== 'set') throw new CloudError('noVault');
}

/** Signing in again as the same account replaces its card; another account gets its own (ADR-0061). */
export function withAccount(list: readonly CloudAccount[], a: CloudAccount): CloudAccount[] {
  return list.some((x) => x.key === a.key) ? list.map((x) => (x.key === a.key ? a : x)) : [...list, a];
}

/** `sleep` for pollFlow on a scope's timers (the client's: they stop with it). */
export function sleepOn(timers: { setTimeout(fn: () => void, ms: number): unknown; clearTimeout(h: never): void }): PollDeps['sleep'] {
  return (ms, signal) =>
    new Promise<void>((resolve, reject) => {
      if (signal.aborted) return reject(new CloudError('cancelled'));
      const h = timers.setTimeout(() => {
        signal.removeEventListener('abort', off);
        resolve();
      }, ms);
      const off = (): void => {
        timers.clearTimeout(h as never);
        reject(new CloudError('cancelled'));
      };
      signal.addEventListener('abort', off, { once: true });
    });
}

/** The screen's list after a sync: saved ones replaced in place (no token: that went to the native side), new ones at
 *  the end, dropped ones gone. */
export function merged(now: readonly Computer[], save: readonly NewComputer[], drop: readonly string[]): Computer[] {
  const bare = (n: NewComputer): Computer => {
    const { token: _t, ...rest } = n as NewComputer & { token?: string };
    return rest as Computer;
  };
  const byId = new Map(save.map((n) => [n.id, bare(n)] as const));
  return [...now.filter((c) => !drop.includes(c.id)).map((c) => byId.get(c.id) ?? c), ...save.filter((n) => !now.some((c) => c.id === n.id)).map(bare)];
}
