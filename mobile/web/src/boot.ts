// The phone's start (plan §7.3). apps/web's entry calls this when it runs inside the Android app,
// instead of connecting to the wisp that served it (on a phone the bundle comes from the APK):
//
//   import { bootPhone, onPhone } from '@wisp/mobile';
//   if (onPhone(window)) {
//     const { client, platform } = await bootPhone({ build: BUILD, createShell: (client, platform) => registerPlugins(client, platform) });
//     render(() => <Shell client={client} platform={platform} />, root);
//   }
//
// `createShell` runs before `client.start`: plugins register their views first, so the restored URL
// may name one (createClient's start). bootPhone then paints from IndexedDB, starts every saved
// computer as a peer, follows the saved list, sends Android's Back to navigation (leaving the app
// when there is nothing to go back to) and writes pending rows and drafts when the app is hidden.
//
// `home` is the launched computer (launch.ts: the one launched last, else the first saved; PHONE, a peer
// never added, only while no computer is saved): the shell runs on it, as v1's app opened that computer's
// wisp, while every saved computer stays connected as its own peer. The app opens on the Computers
// destination (ADR-0065 open question 3's default: v1's), never inside a task; Tasks is one tap away.

import { browserHistory, createClient, indexedDBPersist, sameNav, untracked, uuidv7, type Client, type ClientOptions, type IdbFactory, type IdbKeyRangeStatic, type Target, type WindowLike } from '@wisp/client';
import { androidPlatform, iosPlatform, nativeSeams, type Platform, type PlatformWindow } from '@wisp/shell/platform';
import { COMPUTERS_VIEW, intentOf } from '@wisp/shell/pure';
import { capacitorBridge, exposePlugins, type CapacitorLike, type NativeBridge } from './bridge.ts';
import { migrateV1, type StorageLike } from './migrate.ts';
import { startHome } from './launch.ts';
import { followComputers, phonePeers } from './peers.ts';

export type PhoneWindow = PlatformWindow &
  WindowLike & {
    readonly Capacitor?: PlatformWindow['Capacitor'] & CapacitorLike;
    readonly innerWidth: number;
    readonly localStorage?: StorageLike;
    readonly indexedDB?: IdbFactory;
    readonly IDBKeyRange?: IdbKeyRangeStatic;
  };

export interface PhoneOptions {
  build: string;
  window?: PhoneWindow;
  /** the native side; capacitorBridge(window.Capacitor) by default (tests pass a fake) */
  bridge?: NativeBridge;
  /** registers the plugins' UI before the client starts (the bridge too: the Computers view lists from it) */
  createShell?: (client: Client, platform: Platform, bridge: NativeBridge) => void | Promise<void>;
  /** anything else createClient takes (onError for the toast, dev, warn, timers) */
  client?: Partial<Omit<ClientOptions, 'clientId' | 'build' | 'home'>>;
  /** whether Tasks can open yet (launch.ts tasksReady): Back from Computers leaves the app until it can */
  tasksReady?: () => boolean;
}

export interface Phone {
  readonly client: Client;
  readonly platform: Platform;
  readonly bridge: NativeBridge;
}

/** In the Android app: Capacitor's native bridge is there. */
export function onPhone(w: { readonly Capacitor?: CapacitorLike }): boolean {
  return w.Capacitor?.isNativePlatform?.() === true;
}

// One id per install, so the peers keep this client's subscriptions and pending commands apart.
function clientIdOf(storage: StorageLike | undefined): string {
  const saved = storage?.getItem('wisp.client');
  if (saved) return saved;
  const id = uuidv7();
  storage?.setItem('wisp.client', id);
  return id;
}

const isTarget = (v: unknown): v is Target => {
  const to = typeof v === 'object' && v !== null ? (v as { to?: unknown }).to : undefined;
  return to === 'main' || to === 'side' || to === 'overlay';
};

/** The app's platform (Capacitor says which: the iPhone app or the Android one), with its native seams (secure
 *  storage, the code scanner, push registration) and a tapped notification coming from the native side through the
 *  bridge. Its kind is the look the shell sets on <html> (`data-platform`). */
export function phonePlatform(w: PlatformWindow, bridge: NativeBridge): Platform {
  const kind = w.Capacitor?.getPlatform?.() === 'ios' ? 'ios' : 'android';
  const base = kind === 'ios' ? iosPlatform(w) : androidPlatform(w);
  return { ...base, ...nativeSeams(w.Capacitor, kind), onOpen: (fn) => bridge.onOpen((t) => isTarget(t) && fn(t)) };
}

export async function bootPhone(o: PhoneOptions): Promise<Phone> {
  const w = o.window ?? (globalThis as unknown as PhoneWindow);
  exposePlugins(w.Capacitor); // androidPlatform reads Capacitor.Plugins.{WispSsh,LocalNotifications}
  const bridge = o.bridge ?? capacitorBridge(w.Capacitor);
  const platform = phonePlatform(w, bridge);
  const persist = w.indexedDB && w.IDBKeyRange ? { persist: indexedDBPersist(w.indexedDB, w.IDBKeyRange) } : {};
  await migrateV1(w.localStorage, bridge).catch((err: unknown) => o.client?.warn?.('v1 computers not copied', String(err)));
  const computers = await bridge.list();
  const home = startHome(computers, w.localStorage);
  const client = createClient({ history: browserHistory(w), ...persist, width: w.innerWidth, ...o.client, clientId: clientIdOf(w.localStorage), build: o.build, home });
  await o.createShell?.(client, platform, bridge);
  await client.start(await phonePeers(bridge, computers));
  followComputers(client, bridge, client.scope, computers);
  openOnComputers(client);
  client.scope.add(bridge.onBack(() => back(client, bridge, o.tasksReady)));
  const hidden = (): void => {
    if (w.document.visibilityState === 'hidden') void client.flush();
    else client.peers.resume();
  };
  w.document.addEventListener('visibilitychange', hidden);
  client.scope.add(() => w.document.removeEventListener('visibilitychange', hidden));
  const online = (): void => client.peers.resume();
  w.addEventListener('online', online);
  client.scope.add(() => w.removeEventListener('online', online));
  client.scope.add(platform.onResume(() => client.peers.resume()));
  return { client, platform, bridge };
}

/** The app's start: the Computers destination (when its view is registered), whatever the last place was. */
export function openOnComputers(client: Pick<Client, 'nav' | 'views'>): void {
  if (client.views.accepts('main', COMPUTERS_VIEW, null)) client.nav.navigate(intentOf({ view: COMPUTERS_VIEW }));
}

/** Back closes what's open (an overlay, the side, a task); with nothing left the app goes behind. On Computers while
 *  Tasks can't open yet (no computer opened and connected) it goes behind too: Back never lands on Tasks early. */
export function back(client: Pick<Client, 'nav'>, bridge: Pick<NativeBridge, 'leave'>, tasksReady?: () => boolean): void {
  const before = untracked(client.nav.nav);
  const onComputers = before.main.kind === 'view' && String(before.main.id) === COMPUTERS_VIEW && before.overlays.length === 0 && before.side.kind === 'none';
  if (onComputers && tasksReady && !untracked(tasksReady)) {
    void bridge.leave().catch(() => {});
    return;
  }
  client.nav.navigate({ t: 'back' });
  if (sameNav(before, untracked(client.nav.nav))) void bridge.leave().catch(() => {});
}
