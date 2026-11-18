// The service worker (@wisp/shell src/platform/sw.ts) for the phone and the installed app (v1 cache.js): the page and its
// files come from the cache at once, the network behind them. When the worker finds a new build it
// says so, and the toast's Reload brings it in. In the desktop and Android apps the bundle ships
// inside, so there is nothing to cache.

import type { Platform } from '@wisp/shell';
import type { Toaster } from '@wisp/ui';

export function registerWorker(platform: Platform, toaster: Toaster, t: (key: string) => string): void {
  const sw = navigator.serviceWorker as ServiceWorkerContainer | undefined;
  if (!sw || platform.kind !== 'browser' || !(platform.mobile || platform.standalone)) return;
  sw.register('/sw.js').catch(() => {});
  sw.addEventListener('message', (e: MessageEvent<{ t?: string }>) => {
    // the computer this tab shows isn't connected (QA round 10 Grid #2): reloaded, the grid answers with its waiting
    // page, which opens it again (the worker lets this navigation through to the network)
    if (e.data?.t === 'offline') {
      platform.reload();
      return;
    }
    if (e.data?.t === 'update') toaster.show(t('shell.toast.update'), { duration: 60_000, action: { label: t('shell.toast.reload'), onClick: () => platform.reload() } });
  });
}
