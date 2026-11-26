// The browser: Web Notifications, the Badging API, the Web Share API, an <input type=file> picker,
// navigator.vibrate and the Clipboard API. Each is feature-detected, so Safari, Firefox and an
// insecure origin (http://laptop.local) answer 'unsupported' instead of throwing.

import { decodeNav, encodeNav, type Target } from '@wisp/client';
import type { Haptic, Permission, Platform, PlatformWindow, Shown } from './types.ts';
import { webSafeArea, webWindowLayout } from './web.ts';

const VIBRATE: Record<Haptic, number | number[]> = { tap: 8, select: 4, success: [10, 40, 10], warning: [20, 60, 20], error: [30, 50, 30, 50, 30] };

function permissionOf(w: PlatformWindow): Permission {
  const N = w.Notification;
  if (!N) return 'unsupported';
  return N.permission === 'default' ? 'prompt' : N.permission;
}

/** Where a notification's tap goes, as the page's own URL (nav-url's encoding): the service worker opens it in a new
 *  window when no tab is left, so it must be a link the page reads at start. */
export function targetUrl(t: Target): string {
  const none = { kind: 'none' } as const;
  if (t.to === 'overlay') return encodeNav({ main: { kind: 'home' }, side: none, overlays: [t.overlay] });
  return encodeNav({ main: t.main, side: t.to === 'side' ? t.side : none, overlays: [] });
}

/** The worker's `{t:'open', url}` back as a Target (a broken URL is Home, as decodeNav). */
export function targetOfUrl(url: string): Target {
  const nav = decodeNav(url);
  const top = nav.overlays.at(-1);
  if (top) return { to: 'overlay', overlay: top };
  return nav.side.kind === 'none' ? { to: 'main', main: nav.main } : { to: 'side', main: nav.main, side: nav.side };
}

/** The slice of the service worker container notify reads (a tab it controls: the phone's browser, the installed app). */
interface Registration {
  showNotification(title: string, o: NotificationOptions & { data?: unknown }): Promise<void>;
  getNotifications?(o?: { tag?: string }): Promise<readonly { close(): void }[]>;
}
interface Container {
  readonly controller: unknown;
  readonly ready: Promise<Registration>;
  addEventListener?(type: 'message', fn: (e: MessageEvent<unknown>) => void): void;
}

// size-ok: one object literal of small methods, one per platform call
export function browserPlatform(w: PlatformWindow): Platform {
  const opens = new Set<(t: Target) => void>();
  const nav = w.navigator as Navigator & { setAppBadge?(n: number): Promise<void>; clearAppBadge?(): Promise<void>; standalone?: boolean };
  const coarse = w.matchMedia('(pointer: coarse)').matches;
  const narrow = w.matchMedia('(max-width: 720px)').matches;
  const standalone = w.matchMedia('(display-mode: standalone)').matches || nav.standalone === true;
  const apple = /Mac|iPhone|iPad/.test(nav.platform || nav.userAgent);
  const sw = (nav as { serviceWorker?: Container }).serviceWorker;
  // a tapped note the worker showed (sw.ts notificationclick): the tab it focused is told where to go
  sw?.addEventListener?.('message', (e) => {
    const d = e.data as { t?: unknown; url?: unknown } | null;
    if (!d || typeof d !== 'object' || d.t !== 'open' || typeof d.url !== 'string') return;
    const target = targetOfUrl(d.url);
    for (const fn of opens) fn(target);
  });

  const platform: Platform = {
    kind: 'browser',
    mobile: coarse && narrow,
    phone: false,
    standalone,
    apple,
    titlebar: 'none',
    origin: w.location.origin,
    dragRegions: () => () => {},
    notifyPermission: () => Promise.resolve(permissionOf(w)),
    async requestNotify() {
      const N = w.Notification;
      if (!N) return 'unsupported';
      if (N.permission !== 'default') return permissionOf(w);
      const answer = await N.requestPermission();
      return answer === 'default' ? 'prompt' : answer;
    },
    async notify(n): Promise<Shown> {
      const N = w.Notification;
      if (!N) return 'unsupported';
      if (N.permission !== 'granted') return 'denied';
      const o = { ...(n.body !== undefined && { body: n.body }), ...(n.tag !== undefined && { tag: n.tag }), ...(n.silent !== undefined && { silent: n.silent }) };
      // a page the worker controls shows it through the worker: Chrome on Android has no `new Notification` (it throws
      // 'Illegal constructor'), and a tap reaches the worker even once the tab is gone (sw.ts notificationclick)
      if (sw?.controller) {
        try {
          await (await sw.ready).showNotification(n.title, { ...o, data: { url: n.target ? targetUrl(n.target) : '/' } });
          return 'shown';
        } catch {
          // the worker's way failed: the page's own, below
        }
      }
      let note: Notification;
      try {
        note = new N(n.title, o);
      } catch {
        return 'unsupported';
      }
      const target = n.target;
      note.onclick = () => {
        w.focus?.();
        if (target) for (const fn of opens) fn(target);
        note.close();
      };
      return 'shown';
    },
    // a task opened: its note goes (only the worker's can be found again; the page's own close with their tab)
    async clearNote(tag) {
      if (!sw?.controller) return;
      try {
        for (const note of (await (await sw.ready).getNotifications?.({ tag })) ?? []) note.close();
      } catch {
        // nothing to clear
      }
    },
    async badge(count) {
      if (!nav.setAppBadge) return 'unsupported';
      await (count > 0 ? nav.setAppBadge(count) : (nav.clearAppBadge?.() ?? nav.setAppBadge(0))).catch(() => {});
      return 'set';
    },
    async share(d) {
      if (typeof nav.share !== 'function') return 'unsupported';
      const data: ShareData = {};
      if (d.title !== undefined) data.title = d.title;
      if (d.text !== undefined) data.text = d.text;
      if (d.url !== undefined) data.url = d.url;
      if (d.files) data.files = [...d.files];
      if (data.files && nav.canShare && !nav.canShare(data)) return 'unsupported';
      try {
        await nav.share(data);
        return 'shared';
      } catch (err) {
        return err instanceof DOMException && err.name === 'AbortError' ? 'cancelled' : 'unsupported';
      }
    },
    pickFiles(o = {}) {
      return new Promise((resolve) => {
        const input = w.document.createElement('input');
        input.type = 'file';
        if (o.accept) input.accept = o.accept;
        input.multiple = o.multiple === true;
        if (o.capture) input.setAttribute('capture', 'environment');
        input.addEventListener('change', () => resolve([...(input.files ?? [])]), { once: true });
        input.addEventListener('cancel', () => resolve([]), { once: true });
        input.click();
      });
    },
    haptic(kind) {
      if (typeof nav.vibrate === 'function' && coarse) nav.vibrate(VIBRATE[kind]);
    },
    async copy(text) {
      try {
        await nav.clipboard.writeText(text);
        return true;
      } catch {
        return false;
      }
    },
    openExternal(url) {
      w.open(url, '_blank', 'noopener');
    },
    onResume(fn) {
      const on = (): void => {
        if (w.document.visibilityState === 'visible') fn();
      };
      w.document.addEventListener('visibilitychange', on);
      w.addEventListener('focus', fn);
      return () => {
        w.document.removeEventListener('visibilitychange', on);
        w.removeEventListener('focus', fn);
      };
    },
    reload: () => w.location.reload(),
    safeArea: () => webSafeArea(w),
    windowLayout: () => webWindowLayout(w),
    // a browser has no in-app browser: a tab, as openExternal
    openInApp(url) {
      platform.openExternal(url);
    },
    // a browser's Back is the history's (the nav reads popstate): no gesture progress to pass on
    onBack: () => () => {},
    onOpen(fn) {
      opens.add(fn);
      return () => opens.delete(fn);
    },
  };
  return platform;
}
