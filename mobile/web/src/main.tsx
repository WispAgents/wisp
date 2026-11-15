// The phone app's bundle (ADR-0065 step 8), apart from the browser's (apps/web): `vite build` here writes dist/, which
// the Capacitor apps ship. Every plugin's UI, Settings' frame and the notifier, as the browser entry has them, on the
// shell the launched computer runs. Outside the apps (`npm run dev` here) a stand-in native side and the dev
// server's mock peers stand in for the computers.
import '@wisp/ui/styles';
import { createSignal } from 'solid-js';
import { effect } from '@wisp/client';
import type { Shell } from '@wisp/shell';
import type { Platform } from '@wisp/shell/platform';
import { loadPlatformStyles, type Toaster } from '@wisp/ui';
import { shellSettingsUi } from '@wisp-plugin/shell-settings-ui';
import { startNotifier } from '@wisp-plugin/shell-home-ui';
import { onPhone, type PhoneWindow } from './boot.ts';
import { startPhoneApp } from './app.tsx';
import type { NativeBridge } from './bridge.ts';
import { pluginUis as plugins } from './plugins.ts';


/** Settings (an overlay view, with every plugin's panes) and the notifier, on each shell the app runs. */
function extend(shell: Shell, toaster: Toaster, platform: Platform): () => void {
  type PaneEntry = { pane: ReturnType<ReturnType<typeof shell.settingsPanes>>[number]; client: NonNullable<ReturnType<typeof shell.pluginClient>> };
  const [paneEntries, setPaneEntries] = createSignal<readonly PaneEntry[]>([]);
  const stopPanes = effect(() => {
    setPaneEntries(
      shell.settingsPanes()().flatMap((pane) => {
        const c = shell.pluginClient(pane.plugin);
        return c ? [{ pane, client: c }] : [];
      }),
    );
  });
  shell.register(shellSettingsUi({ panes: paneEntries, t: (key, vars) => shell.client.i18n.t(key, vars) }));
  const home = shell.pluginClient('wisp.shell-home');
  const stopNotify = home ? startNotifier({ client: home, platform, toast: toaster, nav: shell.nav, focused: () => document.hasFocus() }) : undefined;
  return () => {
    stopPanes();
    stopNotify?.();
  };
}

async function main(): Promise<void> {
  const root = document.getElementById('app');
  if (!root) return;
  const w = window as unknown as PhoneWindow & { Capacitor?: { getPlatform?(): string } };
  let bridge: NativeBridge | undefined;
  if (import.meta.env.DEV && !onPhone(w)) {
    // a desktop browser: the stand-in native side (dev only; a production build outside the apps has no computers)
    const { previewBridge, previewLook } = await import('../dev/preview-bridge.ts');
    const look = previewLook(location.search);
    (window as unknown as { Capacitor: { getPlatform(): string } }).Capacitor = { getPlatform: () => look };
    bridge = previewBridge(location.origin, location.search);
  }
  // the look's stylesheet and the phone frame before the first render, so nothing pops in (the shell asks for them too)
  await loadPlatformStyles(w.Capacitor?.getPlatform?.() === 'ios' ? 'ios' : 'android').catch(() => undefined);
  const app = await startPhoneApp({ build: __WISP_BUILD__, root, plugins, extend, dev: import.meta.env.DEV, ...(bridge && { bridge }) });
  // bootPhone writes what's waiting when the app is hidden
  if (import.meta.env.DEV) (window as unknown as { __wispPhone?: unknown }).__wispPhone = app;
}

void main();
