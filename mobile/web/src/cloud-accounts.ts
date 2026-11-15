// wisp Cloud's accounts on the phone (ADR-0065 §2.3-4): the state the Computers screen draws (the accounts, the sign-in
// in progress) and what its buttons do, apart from the DOM so every race is tested (test/cloud.test.ts):
// - signing in runs the device flow (cloud.ts), seals the token, then saves every one of the account's computers again
//   with it (a new token must reach the computers already saved);
// - a refresh waits for the screen's first list, and drops its answer when the account was signed out (or signed in
//   again) meanwhile: each account has a generation that both bump;
// - signing out takes the card and its computers off at once, unseals the token and forgets the computers; a computer
//   the native side couldn't forget comes back on the list (as this phone's own: its card is gone) with a toast.
// The grid has no route to revoke a device token from the device yet (apps/grid edge.rs): signing out is on this phone;
// the grid's account page lists the device to remove it there.
import { signal, type WritableSignal } from '@wisp/client';
import type { Computer, NativeBridge } from './bridge.ts';
import {
  accountComputers,
  accountKey,
  CloudError,
  gone,
  hostOf,
  keepToken,
  listComputers,
  loadAccounts,
  merged,
  pollFlow,
  saveAccounts,
  signInUrl,
  startFlow,
  tokenKey,
  toSave,
  withAccount,
  type CloudAccount,
  type Flow,
  type Provider,
} from './cloud.ts';
import type { CloudDeps } from './ui/Cloud.tsx';

export interface SignIn {
  readonly grid: string;
  readonly via: Provider | undefined;
  /** the grid answered: the code to show */
  readonly flow?: Flow;
  /** it ended in words */
  readonly error?: string;
}

export interface CloudAccountsDeps {
  readonly cloud: CloudDeps;
  readonly bridge: Pick<NativeBridge, 'save' | 'forget'>;
  /** the screen's saved computers now, and how to change them at once */
  readonly list: () => readonly Computer[];
  readonly setList: (fn: (now: readonly Computer[]) => readonly Computer[]) => void;
  /** settles once the screen has the native side's list (a refresh before it would save everything again) */
  readonly ready: Promise<unknown>;
  /** reads the native side's list again (after a save it refused) */
  readonly reload: () => void;
  readonly toast?: { error(m: string): unknown; success(m: string): unknown } | undefined;
  readonly t: (k: string, vars?: Record<string, string>) => string;
}

export interface CloudAccounts {
  readonly accounts: WritableSignal<readonly CloudAccount[]>;
  readonly signIn: WritableSignal<SignIn | undefined>;
  /** every signed-in account's computers again (at start, back in the app) */
  refresh(): Promise<void>;
  /** a provider's button (the hosted grid) or your own grid's Sign in: the sheet at once, the browser once the code comes */
  begin(grid: string, via: Provider | undefined): Promise<void>;
  cancel(): void;
  signOut(key: string): Promise<void>;
  /** the account keys the screen draws a card for (a computer of any other account is this phone's own) */
  known(): ReadonlySet<string>;
  dispose(): void;
}

export function createCloudAccounts(o: CloudAccountsDeps): CloudAccounts {
  const d = o.cloud;
  const t = o.t;
  const accounts = signal<readonly CloudAccount[]>(loadAccounts(d.storage));
  const signIn = signal<SignIn | undefined>(undefined);
  const gens = new Map<string, number>();
  // signed in with a new token whose computers aren't all saved with it yet: every refresh saves them all until one does
  const stale = new Set<string>();
  const gen = (key: string): number => gens.get(key) ?? 0;
  const bump = (key: string): number => {
    const n = gen(key) + 1;
    gens.set(key, n);
    return n;
  };
  let abort: AbortController | undefined;
  let alive = true;

  const keep = (next: readonly CloudAccount[]): void => {
    accounts(next);
    saveAccounts(d.storage, next);
  };
  const has = (key: string): CloudAccount | undefined => accounts().find((a) => a.key === key);
  const mark = (key: string, signedOut: boolean): void => {
    if (has(key)) keep(accounts().map((a) => (a.key === key ? { ...a, signedOut } : a)));
  };

  /** the account's computers as the grid lists them now: saved (with its token) or forgotten here */
  const sync = async (a: CloudAccount, token: string, o2: { box?: { url: string; name: string }; fresh?: boolean } = {}): Promise<void> => {
    const mine = gen(a.key);
    const listed = await listComputers(d.fetch, a.grid, token).catch(() => undefined); // offline: the last list stays
    if (!alive || gen(a.key) !== mine || !has(a.key)) return; // signed out (or in again) meanwhile: not ours to apply
    if (listed === undefined) return;
    if (listed === 'signedOut') return mark(a.key, true);
    const had = o.list();
    const saved = accountComputers(a, had).find((c) => c.kind === 'grid' && !c.computer);
    const box = o2.box ?? (saved?.kind === 'grid' ? { url: saved.url, name: saved.name } : undefined);
    const fresh = o2.fresh === true || stale.has(a.key);
    const save = toSave(a, token, box, listed, had, fresh);
    const drop = gone(a, listed, had);
    if (!save.length && !drop.length) return void stale.delete(a.key);
    o.setList((now) => merged(now, save, drop));
    const out = await Promise.allSettled([...save.map((c) => o.bridge.save(c)), ...drop.map((id) => o.bridge.forget(id))]);
    if (out.some((r) => r.status === 'rejected')) {
      o.toast?.error(t('phone.cloud.saveFailed', { handle: a.handle }));
      o.reload(); // the native side's list as it is
    } else if (gen(a.key) === mine) stale.delete(a.key);
  };

  const refresh = async (): Promise<void> => {
    await o.ready.catch(() => undefined);
    await Promise.all(
      accounts()
        .filter((a) => !a.signedOut)
        .map(async (a) => {
          const token = await d.secure?.get(tokenKey(a.key)).catch(() => null);
          if (!alive || !has(a.key)) return;
          if (!token) return mark(a.key, true);
          await sync(a, token);
        }),
    );
  };

  const message = (err: unknown, grid: string): string => t(`phone.cloud.error.${err instanceof CloudError ? err.code : 'unreachable'}`, { host: hostOf(grid) });

  const begin = async (grid: string, via: Provider | undefined): Promise<void> => {
    abort?.abort(); // the last tap's flow wins
    const ctl = new AbortController();
    abort = ctl;
    signIn({ grid, via }); // the sheet at once, its code to come
    try {
      const flow = await startFlow(d.fetch, grid, { name: d.deviceName, now: d.now(), signal: ctl.signal });
      if (ctl.signal.aborted) return;
      signIn({ grid, via, flow });
      d.openInApp(signInUrl(flow, via, d.look)); // the tap was the choice: no second one
      const g = await pollFlow(flow, { fetch: d.fetch, sleep: d.sleep, now: d.now }, ctl.signal, (wake) => d.onResume(wake));
      // the grid has used up the code: the sign-in finishes even if the sheet closes now, or the token sealed below
      // would belong to no account (or to one whose computers keep the old token)
      const a: CloudAccount = { key: accountKey(grid, g.handle), grid, handle: g.handle, name: g.name, ...(via && { via }) };
      await keepToken(d.secure, a.key, g.token);
      bump(a.key); // a refresh with the old token, still on its way, is dropped
      stale.add(a.key);
      keep(withAccount(accounts(), a));
      if (!alive) return;
      if (abort === ctl || !ctl.signal.aborted) signIn(undefined); // a newer sign-in's sheet stays
      o.toast?.success(t('phone.cloud.signedIn', { handle: g.handle }));
      const box = g.instance.url ? { url: g.instance.url, name: g.instance.name === 'Cloud' ? t('phone.cloud.box') : g.instance.name } : undefined;
      await sync(a, g.token, box ? { box } : {});
    } catch (err) {
      if (ctl.signal.aborted || (err instanceof CloudError && err.code === 'cancelled')) return;
      signIn({ grid, via, error: message(err, grid) });
    } finally {
      if (abort === ctl) abort = undefined;
    }
  };

  const cancel = (): void => {
    abort?.abort();
    abort = undefined;
    signIn(undefined);
  };

  const signOut = async (key: string): Promise<void> => {
    const a = has(key);
    if (!a) return; // a second tap: already on its way
    bump(key);
    stale.delete(key);
    const before = o.list();
    const mine = accountComputers(a, before);
    keep(accounts().filter((x) => x.key !== key)); // the card goes at once
    o.setList((now) => now.filter((c) => !mine.some((m) => m.id === c.id)));
    await d.secure?.remove(tokenKey(key)).catch(() => undefined);
    const out = await Promise.allSettled(mine.map((c) => o.bridge.forget(c.id)));
    const kept = mine.filter((_, i) => out[i]?.status === 'rejected');
    if (kept.length) {
      // still on the phone: back on the list, as this phone's own (the card is gone)
      o.setList((now) => [...now, ...kept.filter((c) => !now.some((x) => x.id === c.id))]);
      o.toast?.error(t('phone.cloud.signOutFailed', { handle: a.handle }));
    } else o.toast?.success(t('phone.cloud.signedOut', { handle: a.handle }));
  };

  return {
    accounts,
    signIn,
    refresh,
    begin,
    cancel,
    signOut,
    known: () => new Set(accounts().map((a) => a.key)),
    dispose: () => {
      alive = false;
      abort?.abort();
    },
  };
}
