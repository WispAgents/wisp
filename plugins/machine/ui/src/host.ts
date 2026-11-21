// What the screens read beyond PluginClient, through the shell's useShell() (packages/shell
// contract.ts ShellContext): navigate, the toaster, the platform. @wisp/shell doesn't export
// useShell() yet, so a Solid context stands in: the shell (or the dev page) provides it, and each
// piece falls back to the PluginClient so the UI still renders on a host that lacks it. One place
// to change once useShell() ships.

import { createContext, useContext } from 'solid-js';
import type { Intent, PluginClient } from '@wisp/client';
import type { ShellContext } from '@wisp/shell/contract';
import { nullPlatform } from '@wisp/shell/platform';
import { createToaster } from '@wisp/ui';

export type Host = Pick<ShellContext, 'navigate' | 'toast' | 'platform'> & { readonly now: () => number };

export const HostContext = createContext<Partial<Host>>({});

export function useHost(client: PluginClient): Host {
  const h = useContext(HostContext);
  return {
    navigate: h.navigate ?? ((i: Intent) => client.navigate(i)),
    // a refused command's toast still shows: the client's onError hands it to the shell's toaster
    toast: h.toast ?? createToaster(),
    platform: h.platform ?? nullPlatform(),
    now: h.now ?? (() => client.scope.timers.now()),
  };
}

export { openSettings } from './open-settings.ts';
