// What the screens read beyond PluginClient: navigate, the toaster, the platform. Inside the shell they're the shell's
// (useShell()); a Solid context stands in on a host without one (the gallery, tests), and each piece falls back to the
// PluginClient so the UI still renders there. The fallback toaster is mounted nowhere: before the shell's was read
// here, every toast of This computer (Freed 1.2 GB, a refused Remove) went to it and never showed.

import { createContext, useContext } from 'solid-js';
import type { Intent, PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import type { ShellContext } from '@wisp/shell/contract';
import { nullPlatform } from '@wisp/shell/platform';
import { createToaster } from '@wisp/ui';

export type Host = Pick<ShellContext, 'navigate' | 'toast' | 'platform'> & { readonly now: () => number };

export const HostContext = createContext<Partial<Host>>({});

/** The shell's context when this renders inside it, else nothing (useShell() says so by throwing). */
function shellOrNone(): Partial<Host> {
  try {
    const s = useShell();
    return { navigate: s.navigate, toast: s.toast, platform: s.platform };
  } catch {
    return {};
  }
}

export function useHost(client: PluginClient): Host {
  const h = { ...shellOrNone(), ...useContext(HostContext) };
  return {
    navigate: h.navigate ?? ((i: Intent) => client.navigate(i)),
    // a refused command's toast still shows: the client's onError hands it to the shell's toaster
    toast: h.toast ?? createToaster(),
    platform: h.platform ?? nullPlatform(),
    now: h.now ?? (() => client.scope.timers.now()),
  };
}

export { openSettings } from './open-settings.ts';
