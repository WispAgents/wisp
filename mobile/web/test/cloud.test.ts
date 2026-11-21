// ADR-0065 step 14, wisp Cloud on the phone (F-phone-5): the grid's device flow run by the app itself, the account's
// computers saved through the native side with its token, and the token sealed (SecureStore), never in web storage.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { FetchLike, FetchResponse } from '@wisp/client';
import type { SecureStore } from '@wisp/shell/platform';
import {
  ACCOUNTS,
  accountComputers,
  accountKey,
  CloudError,
  DEFAULT_GRID,
  gone,
  gridUrl,
  keepToken,
  listComputers,
  loadAccounts,
  loadRevokes,
  pollFlow,
  providersFor,
  saveAccounts,
  savedId,
  signInUrl,
  sleepOn,
  startFlow,
  tellRevoked,
  tokenKey,
  toSave,
  withAccount,
  type CloudAccount,
  type Flow,
} from '../src/cloud.ts';
import type { Computer } from '../src/bridge.ts';
import { createCloudAccounts } from '../src/cloud-accounts.ts';
import { fakeTimers, memoryStorage, settle } from './helpers.ts';

const res = (status: number, body: unknown): FetchResponse => ({
  status,
  ok: status < 400,
  body: null,
  text: async () => JSON.stringify(body),
});

/** a grid answering from a script, recording what it was asked */
function grid(script: Record<string, (body: Record<string, unknown>, headers: Record<string, string>) => FetchResponse | Error>): FetchLike & { asked: string[] } {
  const asked: string[] = [];
  const f = (async (url: string, init: Parameters<FetchLike>[1]) => {
    const path = new URL(url).pathname;
    asked.push(`${init.method} ${path}`);
    const h = script[path];
    if (!h) return res(404, {});
    const out = h(init.body ? (JSON.parse(String(init.body)) as Record<string, unknown>) : {}, init.headers);
    if (out instanceof Error) throw out;
    return out;
  }) as FetchLike & { asked: string[] };
  f.asked = asked;
  return f;
}

const FLOW: Flow = {
  grid: DEFAULT_GRID,
  deviceCode: 'dc',
  userCode: 'AB-CD',
  verify: `${DEFAULT_GRID}/activate?code=AB-CD`,
  interval: 1,
  expiresAt: 1_700_000_600_000,
};
const A: CloudAccount = {
  key: accountKey(DEFAULT_GRID, 'ada'),
  grid: DEFAULT_GRID,
  handle: 'ada',
  name: 'Ada',
};

test('one_tap_providers: GitHub and Google; no Apple until the grid has it', () => {
  assert.deepEqual(providersFor('android'), ['github', 'google']);
  assert.deepEqual(providersFor('ios'), ['github', 'google']);
});

test('your_own_grid_address: https by default, plain http only on loopback, nonsense refused', () => {
  assert.equal(gridUrl('wisp.example.com'), 'https://wisp.example.com');
  assert.equal(gridUrl('https://wisp.example.com/'), 'https://wisp.example.com');
  assert.equal(gridUrl('localhost:4900'), 'http://localhost:4900');
  assert.deepEqual(gridUrl('http://wisp.example.com'), { error: 'insecure' });
  assert.deepEqual(gridUrl('   '), { error: 'empty' });
  assert.deepEqual(gridUrl('not a grid'), { error: 'invalid' });
});

test('the_sign_in_page: the provider returns to /activate with the code and the app, or the grid’s own page', () => {
  assert.equal(signInUrl(FLOW, 'github', 'android'), `${DEFAULT_GRID}/auth/github/start?return=${encodeURIComponent('/activate?code=AB-CD&app=android')}`);
  assert.equal(signInUrl(FLOW, undefined, 'ios'), `${DEFAULT_GRID}/activate?code=AB-CD&app=ios`);
});

test('start_flow_asks_the_grid_for_a_code_and_refuses_what_isnt_a_grid', async () => {
  const g = grid({
    '/api/device/code': (b) => (
      assert.equal(b['name'], 'Phone (wisp app)'),
      res(200, {
        device_code: 'dc',
        user_code: 'AB-CD',
        interval: 3,
        expires_in: 60,
      })
    ),
  });
  const f = await startFlow(g, DEFAULT_GRID, {
    name: 'Phone (wisp app)',
    now: 1000,
  });
  assert.equal(f.userCode, 'AB-CD');
  assert.equal(f.interval, 3);
  assert.equal(f.expiresAt, 61_000);
  await assert.rejects(startFlow(grid({}), DEFAULT_GRID, { name: 'x', now: 0 }), (e: CloudError) => e.code === 'notGrid');
  await assert.rejects(startFlow(grid({ '/api/device/code': () => new Error('offline') }), DEFAULT_GRID, { name: 'x', now: 0 }), (e: CloudError) => e.code === 'unreachable');
});

test('polling_waits_out_pending_slow_down_and_blips_then_hands_over_the_token', async () => {
  const clock = fakeTimers();
  const answers: (FetchResponse | Error)[] = [
    res(400, { error: 'authorization_pending' }),
    new Error('blip'),
    res(502, {}),
    res(400, { error: 'slow_down' }),
    res(200, {
      token: 'wgd_1',
      user: { handle: 'ada', name: 'Ada' },
      instance: { url: 'https://ada.grid.trywisp.dev', name: 'Cloud' },
    }),
  ];
  const waits: number[] = [];
  const g = grid({
    '/api/device/token': (b) => (assert.equal(b['device_code'], 'dc'), answers.shift() ?? res(500, {})),
  });
  const sleep = sleepOn(clock);
  const done = pollFlow(
    { ...FLOW, expiresAt: clock.now() + 600_000 },
    {
      fetch: g,
      sleep: (ms, s) => (waits.push(ms), sleep(ms, s)),
      now: clock.now,
    },
    new AbortController().signal,
  );
  for (let i = 0; i < 12; i++) {
    clock.advance(6000);
    await settle();
  }
  const out = await done;
  assert.equal(out.token, 'wgd_1');
  assert.equal(out.handle, 'ada');
  assert.equal(out.instance.url, 'https://ada.grid.trywisp.dev');
  assert.deepEqual(waits, [1000, 1000, 1000, 1000, 6000], 'slow_down adds 5 s');
});

test('polling_ends_in_words: declined, expired, cancelled', async () => {
  const clock = fakeTimers();
  const run = (answer: FetchResponse, expiresAt = clock.now() + 600_000, ctl = new AbortController()) => {
    const p = pollFlow(
      { ...FLOW, expiresAt },
      {
        fetch: grid({ '/api/device/token': () => answer }),
        sleep: sleepOn(clock),
        now: clock.now,
      },
      ctl.signal,
    );
    p.catch(() => undefined);
    return p;
  };
  const denied = run(res(400, { error: 'access_denied' }));
  const expired = run(res(400, { error: 'expired_token' }));
  const late = run(res(400, { error: 'authorization_pending' }), clock.now() + 500);
  const ctl = new AbortController();
  const cancelled = run(res(400, { error: 'authorization_pending' }), clock.now() + 600_000, ctl);
  ctl.abort();
  clock.advance(1000);
  await settle();
  await assert.rejects(denied, (e: CloudError) => e.code === 'denied');
  await assert.rejects(expired, (e: CloudError) => e.code === 'expired');
  await assert.rejects(late, (e: CloudError) => e.code === 'expired');
  await assert.rejects(cancelled, (e: CloudError) => e.code === 'cancelled');
});

test('back_from_the_browser_asks_at_once: the nudge cuts the wait short', async () => {
  const clock = fakeTimers();
  let wake: (() => void) | undefined;
  const g = grid({
    '/api/device/token': () =>
      res(200, {
        token: 'wgd_1',
        user: { handle: 'ada' },
        instance: { url: 'https://ada.grid.trywisp.dev' },
      }),
  });
  const done = pollFlow(
    { ...FLOW, interval: 30, expiresAt: clock.now() + 600_000 },
    { fetch: g, sleep: sleepOn(clock), now: clock.now },
    new AbortController().signal,
    (fn) => ((wake = fn), () => (wake = undefined)),
  );
  await settle();
  assert.deepEqual(g.asked, [], 'still waiting');
  wake?.();
  await settle();
  assert.equal((await done).token, 'wgd_1');
});

test('the_accounts_computers_are_saved_with_its_token_the_box_first_and_unchanged_ones_left_alone', async () => {
  const g = grid({
    '/api/computers': (_b, h) =>
      h['authorization'] === 'Bearer wgd_1'
        ? res(200, [
            {
              id: 'c1',
              name: 'Work laptop',
              url: 'https://laptop--ada.grid.trywisp.dev',
              online: true,
            },
            { id: 'bad' },
          ])
        : res(401, {}),
  });
  const listed = await listComputers(g, DEFAULT_GRID, 'wgd_1');
  assert.ok(Array.isArray(listed));
  assert.deepEqual(listed, [
    {
      id: 'c1',
      name: 'Work laptop',
      url: 'https://laptop--ada.grid.trywisp.dev',
      online: true,
    },
  ]);
  assert.equal(await listComputers(g, DEFAULT_GRID, 'revoked'), 'signedOut');
  const save = toSave(A, 'wgd_1', { url: 'https://ada.grid.trywisp.dev', name: 'Your cloud wisp' }, listed, []);
  assert.deepEqual(
    save.map((c) => [c.id, c.kind === 'grid' && c.account, c.kind === 'grid' && c.computer, 'token' in c && c.token]),
    [
      [savedId(A, 'box'), A.key, undefined, 'wgd_1'],
      [savedId(A, 'c1'), A.key, 'c1', 'wgd_1'],
    ],
  );
  const saved = save.map((n) => {
    const { token: _t, ...c } = n as Computer & { token?: string };
    return c as Computer;
  });
  assert.deepEqual(toSave(A, 'wgd_1', { url: 'https://ada.grid.trywisp.dev', name: 'Your cloud wisp' }, listed, saved), [], 'nothing changed: nothing saved');
  assert.deepEqual(
    accountComputers(A, saved).map((c) => c.id),
    [savedId(A, 'box'), savedId(A, 'c1')],
  );
  assert.deepEqual(gone(A, [], saved), [savedId(A, 'c1')], 'a computer the grid no longer lists goes; the box stays');
});

test('an_older_grids_list_shape_still_reads', async () => {
  const g = grid({
    '/api/computers': () =>
      res(200, {
        computers: [{ id: 'c1', name: 'Tower', url: 'https://t', online: false }],
      }),
  });
  assert.deepEqual(await listComputers(g, DEFAULT_GRID, 't'), [{ id: 'c1', name: 'Tower', url: 'https://t', online: false }]);
});

test('tokens_never_land_in_web_storage: the account list keeps no secret, the token is sealed or refused', async () => {
  const storage = memoryStorage();
  saveAccounts(storage, [{ ...A, via: 'github', token: 'wgd_secret' } as CloudAccount]);
  const raw = storage.getItem(ACCOUNTS) ?? '';
  assert.ok(!raw.includes('wgd_secret'));
  assert.deepEqual(loadAccounts(storage), [{ ...A, via: 'github' }]);
  const sealed = new Map<string, string>();
  const secure: SecureStore = {
    get: async (k) => sealed.get(k) ?? null,
    set: async (k, v) => (sealed.set(k, v), 'set'),
    remove: async (k) => void sealed.delete(k),
  };
  await keepToken(secure, A.key, 'wgd_secret');
  assert.equal(sealed.get(tokenKey(A.key)), 'wgd_secret');
  const none: SecureStore = {
    get: async () => null,
    set: async () => 'unsupported',
    remove: async () => {},
  };
  await assert.rejects(keepToken(none, A.key, 'wgd_secret'), (e: CloudError) => e.code === 'noVault');
  await assert.rejects(keepToken(undefined, A.key, 'wgd_secret'), (e: CloudError) => e.code === 'noVault');
  for (const v of storage.map.values()) assert.ok(!v.includes('wgd_secret'));
});

test('several_accounts: the same one again replaces its card, another gets its own', () => {
  const B: CloudAccount = {
    ...A,
    key: accountKey(DEFAULT_GRID, 'bob'),
    handle: 'bob',
    name: 'Bob',
  };
  assert.deepEqual(
    withAccount([A], { ...A, name: 'Ada L.' }).map((a) => a.name),
    ['Ada L.'],
  );
  assert.deepEqual(
    withAccount([A], B).map((a) => a.handle),
    ['ada', 'bob'],
  );
});

// --- the accounts as the Computers screen runs them (cloud-accounts.ts): the races a sign-in and a sign-out meet ---

type Saved = { kind: 'save' | 'forget' | 'revoke'; id: string; token?: string };

function harness(o: { failForget?: boolean; secure?: SecureStore | null; list?: Computer[] } = {}) {
  const clock = fakeTimers();
  const sealed = new Map<string, string>();
  const secure: SecureStore | undefined =
    o.secure === null
      ? undefined
      : (o.secure ?? { get: async (k) => sealed.get(k) ?? null, set: async (k, v) => (sealed.set(k, v), 'set'), remove: async (k) => void sealed.delete(k) });
  const storage = memoryStorage();
  let token = 'wgd_1';
  let computers: { id: string; name: string; url: string }[] = [{ id: 'c1', name: 'Work laptop', url: 'https://laptop--ada.grid.trywisp.dev' }];
  let hold: Promise<void> | undefined; // a /api/computers answer held back
  let holdRevoke: Promise<void> | undefined; // a /api/device/revoke answer held back
  let down = false; // the grid can't be reached for a revoke
  const log: Saved[] = [];
  const g = grid({
    '/api/device/code': () => res(200, { device_code: 'dc', user_code: 'AB-CD', interval: 1, expires_in: 600 }),
    '/api/device/token': () => res(200, { token, user: { handle: 'ada', name: 'Ada' }, instance: { url: 'https://ada.grid.trywisp.dev', name: 'Cloud' } }),
    '/api/computers': (_b, h) => (h['authorization'] === `Bearer ${token}` ? res(200, computers) : res(401, {})),
    '/api/device/revoke': (_b, h) => {
      if (down) return new Error('offline');
      log.push({ kind: 'revoke', id: '', token: (h['authorization'] ?? '').replace('Bearer ', '') });
      return res(200, { ok: true, revoked: true });
    },
  });
  const fetch: FetchLike = async (url, init) => {
    if (new URL(url).pathname === '/api/computers' && hold) await hold;
    if (new URL(url).pathname === '/api/device/revoke' && holdRevoke) await holdRevoke;
    return g(url, init);
  };
  const opened: string[] = [];
  const resumes = new Set<() => void>();
  const toasts: string[] = [];
  let list: readonly Computer[] = o.list ?? [];
  let ready!: () => void;
  const readyP = new Promise<void>((r) => (ready = r));
  const cloud = createCloudAccounts({
    cloud: {
      look: 'android',
      fetch,
      secure,
      storage,
      sleep: sleepOn(clock),
      now: clock.now,
      openInApp: (u) => void opened.push(u),
      openExternal: () => {},
      onResume: (fn) => (resumes.add(fn), () => resumes.delete(fn)),
      deviceName: 'Phone (wisp app)',
    },
    bridge: {
      save: async (c) => void log.push({ kind: 'save', id: c.id, ...('token' in c && { token: c.token }) }),
      forget: async (id) => {
        log.push({ kind: 'forget', id });
        if (o.failForget) throw new Error('no');
      },
    },
    list: () => list,
    setList: (fn) => void (list = fn(list)),
    ready: readyP,
    reload: () => {},
    toast: { error: (m: string) => void toasts.push(`error: ${m}`), success: (m: string) => void toasts.push(m) },
    t: (k, v) => (v ? `${k} ${JSON.stringify(v)}` : k),
  });
  const run = async (): Promise<void> => {
    for (let i = 0; i < 6; i++) {
      clock.advance(1000);
      await settle();
    }
  };
  return {
    cloud,
    clock,
    sealed,
    storage,
    log,
    toasts,
    opened,
    run,
    ready,
    list: () => list,
    setToken: (t: string) => void (token = t),
    setDown: (v: boolean) => void (down = v),
    setComputers: (c: typeof computers) => void (computers = c),
    holdRevoke: () => {
      let release!: () => void;
      holdRevoke = new Promise<void>((r) => (release = r));
      return () => {
        holdRevoke = undefined;
        release();
      };
    },
    hold: () => {
      let release!: () => void;
      hold = new Promise<void>((r) => (release = r));
      return () => {
        hold = undefined;
        release();
      };
    },
  };
}

test('signing_in_opens_the_provider_seals_the_token_and_saves_the_box_and_computers_with_it', async () => {
  const h = harness();
  const done = h.cloud.begin(DEFAULT_GRID, 'github');
  assert.deepEqual(h.cloud.signIn(), { grid: DEFAULT_GRID, via: 'github' }, 'the sheet at once, before the grid answers');
  await h.run();
  await done;
  assert.equal(h.opened[0], `${DEFAULT_GRID}/auth/github/start?return=${encodeURIComponent('/activate?code=AB-CD&app=android')}`);
  assert.equal(h.cloud.signIn(), undefined);
  assert.deepEqual(
    h.cloud.accounts().map((a) => a.key),
    [A.key],
  );
  assert.equal(h.sealed.get(tokenKey(A.key)), 'wgd_1');
  assert.deepEqual(h.log, [
    { kind: 'save', id: savedId(A, 'box'), token: 'wgd_1' },
    { kind: 'save', id: savedId(A, 'c1'), token: 'wgd_1' },
  ]);
  assert.deepEqual(
    h.list().map((c) => c.id),
    [savedId(A, 'box'), savedId(A, 'c1')],
  );
  assert.ok(![...h.storage.map.values()].some((v) => v.includes('wgd_1')), 'no token in web storage');
});

test('signing_in_again_saves_every_computer_again_with_the_new_token', async () => {
  const h = harness();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  h.setToken('wgd_2');
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  assert.deepEqual(
    h.log.map((s) => [s.id, s.token]),
    [
      [savedId(A, 'box'), 'wgd_2'],
      [savedId(A, 'c1'), 'wgd_2'],
    ],
  );
  assert.equal(h.cloud.accounts().length, 1, 'the same account: one card');
});

test('a_refresh_waits_for_the_screens_list_and_a_revoked_token_marks_the_card_signed_out', async () => {
  const h = harness();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  const r = h.cloud.refresh();
  await settle();
  assert.deepEqual(h.log, [], 'nothing before the native list is in');
  h.ready();
  await r;
  assert.deepEqual(h.log, [], 'unchanged: nothing saved again');
  h.setToken('wgd_other'); // the grid revoked ours
  await h.cloud.refresh();
  assert.equal(h.cloud.accounts()[0]?.signedOut, true);
  assert.equal(h.list().length, 2, 'its computers stay, read-only');
});

test('a_computer_removed_on_the_grid_is_forgotten_here_on_refresh', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  h.setComputers([]);
  await h.cloud.refresh();
  assert.deepEqual(h.log, [{ kind: 'forget', id: savedId(A, 'c1') }]);
  assert.deepEqual(
    h.list().map((c) => c.id),
    [savedId(A, 'box')],
  );
});

test('signing_out_during_a_refresh_brings_nothing_back', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.setComputers([
    { id: 'c1', name: 'Work laptop', url: 'https://laptop--ada.grid.trywisp.dev' },
    { id: 'c2', name: 'Tower', url: 'https://t' },
  ]);
  const release = h.hold();
  const r = h.cloud.refresh();
  await settle();
  await h.cloud.signOut(A.key);
  assert.deepEqual(h.list(), [], 'gone at once');
  release();
  await r;
  assert.deepEqual(h.list(), [], 'the refresh’s answer is dropped');
  assert.ok(!h.log.some((s) => s.kind === 'save' && s.id === savedId(A, 'c2')));
  assert.equal(h.sealed.size, 0, 'the token is unsealed');
  assert.deepEqual(h.cloud.accounts(), []);
});

test('signing_out_twice_does_it_once_and_a_computer_the_phone_couldnt_forget_stays_listed', async () => {
  const h = harness({ failForget: true });
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  h.toasts.length = 0;
  await Promise.all([h.cloud.signOut(A.key), h.cloud.signOut(A.key)]);
  assert.equal(h.log.filter((s) => s.kind === 'forget').length, 2, 'two computers, forgotten once each');
  assert.deepEqual(
    h.list().map((c) => c.id),
    [savedId(A, 'box'), savedId(A, 'c1')],
    'still on the phone: listed again',
  );
  assert.equal(h.toasts.length, 1);
  assert.ok(h.toasts[0]?.startsWith('error: phone.cloud.signOutFailed'));
});

test('sign_out_calls_revoke_then_forgets: the grid revokes the token before the phone lets go of it', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  h.toasts.length = 0;
  await h.cloud.signOut(A.key);
  assert.deepEqual(
    h.log.map((s) => `${s.kind} ${s.token ?? s.id}`),
    ['revoke wgd_1', `forget ${savedId(A, 'box')}`, `forget ${savedId(A, 'c1')}`],
    'revoked with its own token first, then forgotten',
  );
  assert.equal(h.sealed.size, 0, 'unsealed');
  assert.deepEqual(h.toasts, [`phone.cloud.signedOut ${JSON.stringify({ handle: 'ada' })}`]);
});

test('sign_out_offline_queues_the_revoke: forgotten here at once, told on the next connect, and says so', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  h.toasts.length = 0;
  h.setDown(true);
  await h.cloud.signOut(A.key);
  assert.deepEqual(h.cloud.accounts(), [], 'the card is gone all the same');
  assert.deepEqual(h.list(), []);
  assert.ok(!h.log.some((s) => s.kind === 'revoke'));
  assert.equal(h.sealed.has(tokenKey(A.key)), false, 'no longer the account’s token');
  const queued = loadRevokes(h.storage);
  assert.equal(queued.length, 1);
  assert.equal(queued[0]?.grid, DEFAULT_GRID);
  assert.equal(h.sealed.get(queued[0]?.slot ?? ''), 'wgd_1', 'kept sealed to tell the grid later');
  assert.ok(![...h.storage.map.values()].some((v) => v.includes('wgd_1')), 'no token in web storage');
  assert.equal(h.toasts.length, 1);
  assert.ok(h.toasts[0]?.startsWith('phone.cloud.signedOutLater'), String(h.toasts[0]));
  assert.ok(h.toasts[0]?.includes('grid.trywisp.dev'));

  // still offline: the next refresh keeps it
  await h.cloud.refresh();
  await settle();
  assert.equal(loadRevokes(h.storage).length, 1);

  // back online: the next refresh tells the grid and forgets it
  h.setDown(false);
  await h.cloud.refresh();
  await settle();
  assert.deepEqual(
    h.log.filter((s) => s.kind === 'revoke').map((s) => s.token),
    ['wgd_1'],
  );
  assert.deepEqual(loadRevokes(h.storage), []);
  assert.equal(h.sealed.size, 0);
});

test('signing_in_again_while_the_grid_is_told_keeps_the_new_sign_in', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  const release = h.holdRevoke();
  const out = h.cloud.signOut(A.key);
  await settle();
  h.setToken('wgd_2');
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  release();
  await out;
  await settle();
  assert.equal(h.sealed.get(tokenKey(A.key)), 'wgd_2', 'the new token stays');
  assert.deepEqual(
    h.cloud.accounts().map((a) => a.key),
    [A.key],
  );
  assert.ok(!h.log.some((s) => s.kind === 'forget'), 'its computers, saved again, aren’t forgotten');
  assert.deepEqual(
    h.log.filter((s) => s.kind === 'revoke').map((s) => s.token),
    ['wgd_1'],
    'only the old token revoked',
  );
  assert.deepEqual(loadRevokes(h.storage), [], 'and let go once told');
});

test('a_sign_out_is_written_ahead: the token is set aside before the grid is asked', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  const release = h.holdRevoke();
  const out = h.cloud.signOut(A.key);
  await settle();
  // the app killed here: the next start still knows to tell the grid
  assert.equal(loadRevokes(h.storage).length, 1);
  assert.equal(h.sealed.has(tokenKey(A.key)), false);
  release();
  await out;
  assert.deepEqual(loadRevokes(h.storage), []);
});

test('a_sealed_store_that_fails_once_keeps_the_queued_revoke', async () => {
  const sealed = new Map<string, string>();
  let failGet = false;
  const secure: SecureStore = {
    get: async (k) => {
      if (failGet && k.startsWith('wisp.cloud.revoke.')) throw new Error('locked');
      return sealed.get(k) ?? null;
    },
    set: async (k, v) => (sealed.set(k, v), 'set'),
    remove: async (k) => void sealed.delete(k),
  };
  const h = harness({ secure });
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.setDown(true);
  await h.cloud.signOut(A.key);
  h.setDown(false);
  failGet = true;
  await h.cloud.refresh();
  await settle();
  assert.equal(loadRevokes(h.storage).length, 1, 'kept for the next time');
  failGet = false;
  await h.cloud.refresh();
  await settle();
  assert.deepEqual(loadRevokes(h.storage), []);
  assert.equal(sealed.size, 0);
});

test('tell_revoked: done on the grid’s own ok, 401/403 and an older grid’s 404; again later on anything else', async () => {
  const at = (status: number | Error) => grid({ '/api/device/revoke': () => (status instanceof Error ? status : res(status, {})) });
  const ok = grid({ '/api/device/revoke': () => res(200, { ok: true, revoked: false }) });
  assert.equal(await tellRevoked(ok, DEFAULT_GRID, 'wgd_1'), true, 'the grid’s own 200');
  for (const s of [401, 403, 404]) assert.equal(await tellRevoked(at(s), DEFAULT_GRID, 'wgd_1'), true, String(s));
  // a captive portal's 200 page, a proxy's redirect or 421, and the grid unwell or away: asked again later
  for (const s of [200, 302, 421, 500, 503, 429, new Error('offline')]) assert.equal(await tellRevoked(at(s), DEFAULT_GRID, 'wgd_1'), false, String(s));
  // a grid that never answers: the wait ends it
  const clock = fakeTimers();
  const hang: FetchLike = (_u, init) => new Promise((_r, reject) => init.signal?.addEventListener('abort', () => reject(new Error('aborted'))));
  const p = tellRevoked(hang, DEFAULT_GRID, 'wgd_1', sleepOn(clock), 10_000);
  clock.advance(10_000);
  assert.equal(await p, false);
});

test('the_last_tap_wins_and_cancel_stops_the_poll', async () => {
  const h = harness();
  const first = h.cloud.begin(DEFAULT_GRID, 'github');
  const second = h.cloud.begin(DEFAULT_GRID, 'google');
  await h.run();
  await Promise.all([first, second]);
  assert.equal(h.opened.length, 1, 'one browser');
  assert.ok(h.opened[0]?.includes('/auth/google/'));
  assert.equal(h.cloud.accounts()[0]?.via, 'google');
  const h2 = harness();
  void h2.cloud.begin(DEFAULT_GRID, 'github');
  await settle();
  h2.cloud.cancel();
  await h2.run();
  assert.equal(h2.cloud.signIn(), undefined);
  assert.deepEqual(h2.cloud.accounts(), [], 'cancelled: no account');
});

test('a_phone_without_a_sealed_store_says_so_and_keeps_nothing', async () => {
  const h = harness({ secure: null });
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  assert.equal(h.cloud.signIn()?.error, 'phone.cloud.error.noVault {"host":"grid.trywisp.dev"}');
  assert.deepEqual(h.cloud.accounts(), []);
  assert.deepEqual(h.log, []);
});

test('a_new_token_reaches_every_computer_even_when_the_first_list_after_signing_in_fails', async () => {
  const h = harness();
  h.ready();
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  h.log.length = 0;
  h.setToken('wgd_2');
  const release = h.hold(); // the list after the new sign-in doesn't come (offline)
  void h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run();
  assert.equal(h.log.length, 0, 'nothing saved yet');
  release();
  await h.run();
  await h.cloud.refresh();
  assert.deepEqual(
    h.log.filter((s) => s.kind === 'save').map((s) => [s.id, s.token]),
    [
      [savedId(A, 'box'), 'wgd_2'],
      [savedId(A, 'c1'), 'wgd_2'],
    ],
  );
  h.log.length = 0;
  await h.cloud.refresh();
  assert.deepEqual(h.log, [], 'saved once with it: the next refresh leaves them');
});

test('closing_the_sheet_once_the_grid_said_yes_still_finishes_the_sign_in', async () => {
  let release!: () => void;
  const gate = new Promise<void>((r) => (release = r));
  const sealed = new Map<string, string>();
  const h = harness({
    secure: { get: async (k) => sealed.get(k) ?? null, set: async (k, v) => (await gate, sealed.set(k, v), 'set'), remove: async (k) => void sealed.delete(k) },
  });
  h.ready();
  const done = h.cloud.begin(DEFAULT_GRID, 'github');
  await h.run(); // the token came; sealing it waits
  h.cloud.cancel();
  release();
  await h.run();
  await done;
  assert.deepEqual(
    h.cloud.accounts().map((a) => a.key),
    [A.key],
  );
  assert.equal(sealed.get(tokenKey(A.key)), 'wgd_1');
  assert.ok(h.log.some((s) => s.id === savedId(A, 'c1') && s.token === 'wgd_1'));
});
