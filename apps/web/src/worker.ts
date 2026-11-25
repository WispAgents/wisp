// The service worker (@wisp/shell src/platform/sw.ts) for the phone and the installed app (v1 cache.js): the page and its
// files come from the cache at once, the network behind them. When the worker finds a new build it
// says so, and the toast's Reload brings it in. In the desktop and Android apps the bundle ships
// inside, so there is nothing to cache.
// The worker looks for a new build on a navigation only, and an installed app left open (or brought back from the
// background) navigates for days: so the tab asks it to look (`{t:'check'}`) when it comes back into view, when the
// app resumes and when its stream comes back live (the computer may have been updated meanwhile). The worker
// throttles them (one at a time, one per half minute).

import { subscribe, type Readable } from '@wisp/client';
import type { Platform } from '@wisp/shell';
import type { Toaster } from '@wisp/ui';

export interface WorkerHooks {
  /** whether the page's own stream is live: its rise (not live → live) asks for a check */
  readonly live?: Readable<boolean>;
  /** tests: the container and the document (navigator.serviceWorker, document) */
  readonly container?: ServiceWorkerContainer;
  readonly doc?: Pick<Document, 'visibilityState' | 'addEventListener' | 'removeEventListener'>;
}

/** Returns what takes its listeners off again. */
export function registerWorker(platform: Platform, toaster: Toaster, t: (key: string) => string, o: WorkerHooks = {}): () => void {
  const sw = o.container ?? (navigator.serviceWorker as ServiceWorkerContainer | undefined);
  if (!sw || platform.kind !== 'browser' || !(platform.mobile || platform.standalone)) return () => {};
  sw.register('/sw.js').catch(() => {});
  const onMessage = (e: MessageEvent<{ t?: string }>): void => {
    // the computer this tab shows isn't connected (QA round 10 Grid #2): reloaded, the grid answers with its waiting
    // page, which opens it again (the worker lets this navigation through to the network)
    if (e.data?.t === 'offline') {
      platform.reload();
      return;
    }
    if (e.data?.t === 'update') toaster.show(t('shell.toast.update'), { duration: 60_000, action: { label: t('shell.toast.reload'), onClick: () => platform.reload() } });
  };
  sw.addEventListener('message', onMessage as EventListener);
  const check = (): void => sw.controller?.postMessage({ t: 'check' });
  const doc = o.doc ?? document;
  const onVisible = (): void => {
    if (doc.visibilityState === 'visible') check();
  };
  doc.addEventListener('visibilitychange', onVisible);
  const offResume = platform.onResume(check);
  const live = o.live;
  const offLive = live ? subscribe(live, (now) => void (now && check())) : () => {};
  return () => {
    sw.removeEventListener('message', onMessage as EventListener);
    doc.removeEventListener('visibilitychange', onVisible);
    offResume();
    offLive();
  };
}
