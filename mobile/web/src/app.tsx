// The phone app (ADR-0065 step 8): bootPhone's client and peers, the shell on the launched computer with the platform
// look, every plugin's UI and the app's own Computers destination. Launching another computer runs the shell again on
// it (what's mounted read the old home); the client, and with it every peer's connection, rows and pending commands,
// stays. No app CSS: the shell sets `data-platform` and the kit draws Material 3 or HIG.
import type { Client, FetchLike } from '@wisp/client';
import { commandErrorToast, createShell, type PluginUiDef, type Shell } from '@wisp/shell';
import type { Platform, SecureStore } from '@wisp/shell/platform';
import { createToaster, type Toaster } from '@wisp/ui';
import type { NativeBridge } from './bridge.ts';
import { bootPhone, type PhoneWindow } from './boot.ts';
import { createLauncher, type Launcher } from './launch.ts';
import { phoneUi } from './ui/index.tsx';
import { followStayConnected, type StayConnected } from './stay.ts';
import { sleepOn } from './cloud.ts';
import type { StorageLike } from './migrate.ts';
import type { CloudDeps } from './ui/Cloud.tsx';

export interface PhoneAppOptions {
  build: string;
  root: HTMLElement;
  window?: PhoneWindow;
  /** the native side (tests and the preview pass a fake) */
  bridge?: NativeBridge;
  /** every plugin's UI, in registration order */
  plugins: () => readonly { readonly path: string; readonly def: PluginUiDef | undefined }[];
  /** what the entry adds to each shell it runs (Settings' frame, the notifier); returns how to stop it */
  extend?: (shell: Shell, toaster: Toaster, platform: Platform) => (() => void) | void;
  dev?: boolean;
  /** wisp Cloud's network and sealed store; default: the WebView's fetch and the platform's SecureStore (the preview
   *  passes a mock grid and an in-memory store) */
  cloud?: { readonly fetch?: FetchLike; readonly secure?: SecureStore };
}

/** wisp Cloud on the Computers screen: the platform's browser, sealed store and resume, the client's timers. */
export function cloudDeps(client: Client, platform: Platform, storage: StorageLike | undefined, o: PhoneAppOptions['cloud'] = {}): CloudDeps {
  const mem = new Map<string, string>(); // no web storage: the accounts last as long as the app
  const look = platform.kind === 'ios' ? 'ios' : 'android';
  return {
    look,
    fetch: o.fetch ?? ((url, init) => globalThis.fetch(url, init)),
    secure: o.secure ?? platform.secureStore,
    storage: storage ?? { getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => void mem.set(k, v) },
    sleep: sleepOn(client.scope.timers),
    now: () => client.scope.timers.now(),
    openInApp: (url) => platform.openInApp(url),
    openExternal: (url) => platform.openExternal(url),
    onResume: (fn) => platform.onResume(fn),
    copy: (text) => platform.copy(text),
    deviceName: look === 'ios' ? 'iPhone (wisp app)' : 'Phone (wisp app)',
  };
}

export interface PhoneApp {
  readonly client: Client;
  readonly platform: Platform;
  readonly launcher: Launcher;
  /** the shell running now (another one after each switch) */
  shell(): Shell;
}

export async function startPhoneApp(o: PhoneAppOptions): Promise<PhoneApp> {
  const w = o.window ?? (globalThis as unknown as PhoneWindow);
  const toaster = createToaster();
  let shell: Shell | undefined;
  let stopExtend: (() => void) | void;
  let launcher: Launcher | undefined;
  let stay: StayConnected | undefined;

  const make = (client: Client, platform: Platform, bridge: NativeBridge): Shell => {
    const s = createShell({ client, toaster, platform, home: client.home(), window: w as unknown as Parameters<typeof createShell>[0]['window'] });
    for (const { path, def } of o.plugins()) {
      if (!def) continue;
      try {
        s.register(def);
      } catch (err) {
        client.devtools.record({ kind: 'warn', message: `${path}: ${String(err)}` }); // one bad plugin never stops the app
      }
    }
    if (launcher) {
      const scan = platform.scanCode?.bind(platform);
      const cloud = cloudDeps(client, platform, w.localStorage, o.cloud);
      s.register(phoneUi({ bridge, launcher, toast: toaster, newId: () => crypto.randomUUID(), cloud, ...(scan && { scan }), ...(stay && { stay }) }));
    }
    stopExtend = o.extend?.(s, toaster, platform);
    return s;
  };

  let stopRender: (() => void) | undefined;
  const phone = await bootPhone({
    build: o.build,
    window: w,
    ...(o.bridge && { bridge: o.bridge }),
    client: { onError: commandErrorToast(toaster), ...(o.dev !== undefined && { dev: o.dev }) },
    createShell: (client, platform, bridge) => {
      // Stay connected follows the saved computers (ON for SSH without push) for as long as the client lives
      stay = followStayConnected(bridge, w.localStorage, client.scope);
      launcher = createLauncher({
        client,
        storage: w.localStorage,
        scope: client.scope,
        // after the press that launched it has finished: the old shell isn't torn down under its own handler
        onSwitch: () =>
          queueMicrotask(() => {
            stopRender?.();
            stopExtend?.();
            shell?.dispose();
            shell = make(client, platform, bridge);
            stopRender = shell.render(o.root);
          }),
      });
      shell = make(client, platform, bridge);
    },
  });
  if (!shell || !launcher) throw new Error('the phone app’s shell was not made');
  stopRender = shell.render(o.root);
  const current = (): Shell => {
    if (!shell) throw new Error('no shell');
    return shell;
  };
  // the next shell's effects and listeners go with the client
  phone.client.scope.add(() => {
    stopRender?.();
    stopExtend?.();
  });
  return { client: phone.client, platform: phone.platform, launcher, shell: current };
}
