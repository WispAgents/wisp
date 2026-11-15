// The phone app (ADR-0065 step 8): bootPhone's client and peers, the shell on the launched computer with the platform
// look, every plugin's UI and the app's own Computers destination. Launching another computer runs the shell again on
// it (what's mounted read the old home); the client, and with it every peer's connection, rows and pending commands,
// stays. No app CSS: the shell sets `data-platform` and the kit draws Material 3 or HIG.
import type { Client } from '@wisp/client';
import { commandErrorToast, createShell, type PluginUiDef, type Shell } from '@wisp/shell';
import type { Platform } from '@wisp/shell/platform';
import { createToaster, type Toaster } from '@wisp/ui';
import type { NativeBridge } from './bridge.ts';
import { bootPhone, type PhoneWindow } from './boot.ts';
import { createLauncher, type Launcher } from './launch.ts';
import { phoneUi } from './ui/index.tsx';

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
    if (launcher) s.register(phoneUi({ bridge, launcher }));
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
