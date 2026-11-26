// What the two Capacitor phone apps (Android, iPhone: ADR-0065) share: the app's own share sheet (WispSsh.share), the
// stock plugins (Share, Haptics, LocalNotifications, Browser) when the build has them, the native Back gesture's
// `wisp-back` events, and the browser's answer for anything a build lacks. Each app's module adds what only it has.

import { browserPlatform } from './browser.ts';
import type { BackEvent, CapacitorGlobal, Haptic, Platform, PlatformWindow } from './types.ts';
import { backEventOf } from './web.ts';

type PluginFn = (args?: unknown) => Promise<unknown>;

export function pluginFn(cap: CapacitorGlobal | undefined, plugin: string, method: string): PluginFn | undefined {
  return cap?.Plugins?.[plugin]?.[method];
}

const IMPACT: Record<Haptic, string> = { tap: 'LIGHT', select: 'LIGHT', success: 'MEDIUM', warning: 'MEDIUM', error: 'HEAVY' };

/** The native side's predictive Back (Android) / edge swipe (iOS), dispatched on the window as `wisp-back`
 * CustomEvents ({phase, edge, progress}). An older app sends none: Back stays the history's. */
export function nativeBack(w: PlatformWindow): Platform['onBack'] {
  return (fn: (e: BackEvent) => void) => {
    const on = (e: Event): void => {
      const b = backEventOf((e as CustomEvent<unknown>).detail);
      if (b) fn(b);
    };
    w.addEventListener('wisp-back', on);
    return () => w.removeEventListener('wisp-back', on);
  };
}

/** A phone app's platform: the browser's, with the Capacitor plugins it has. `openExternal` is the app's own. */
export function capacitorPlatform(w: PlatformWindow, kind: 'android' | 'ios', openExternal: (url: string) => void): Platform {
  const base = browserPlatform(w);
  const cap = w.Capacitor;
  const call = async (plugin: string, method: string, args?: unknown): Promise<unknown> => {
    const fn = pluginFn(cap, plugin, method);
    if (!fn) throw new Error(`${plugin}.${method} is not in this build`);
    return fn(args);
  };
  return {
    ...base,
    kind,
    mobile: true,
    phone: true,
    standalone: true,
    async notify(n) {
      const fn = pluginFn(cap, 'LocalNotifications', 'schedule');
      if (!fn) return base.notify(n);
      // the tag is the note's id, so a task's later note replaces its earlier one: the Android app's plugin
      // (Notifications.java) posts under a string tag; a stock plugin (the iPhone's) takes a number only
      const id = kind === 'android' ? (n.tag ?? String(Math.floor(Math.random() * 2 ** 31))) : Math.floor(Math.random() * 2 ** 31);
      await fn({ notifications: [{ id, title: n.title, body: n.body ?? '', extra: n.target ?? null }] });
      return 'shown';
    },
    async clearNote(tag) {
      const fn = pluginFn(cap, 'LocalNotifications', 'removeDelivered');
      if (!fn || kind !== 'android') return;
      await fn({ notifications: [{ id: tag }] }).catch(() => {});
    },
    // the app's own share sheet first (WispSsh.share: Android's chooser; its System WebView has no navigator.share and
    // the build carries no Share plugin, so every Share was a silent copy), then @capacitor/share, then the browser's
    async share(d) {
      const own = pluginFn(cap, 'WispSsh', 'share');
      // (text and links only: a file goes the stock plugin's or the browser's way)
      if (own && !d.files?.length) {
        try {
          const r = await own({ title: d.title, text: d.text, url: d.url });
          return r && typeof r === 'object' && (r as { cancelled?: unknown }).cancelled === true ? 'cancelled' : 'shared';
        } catch {
          // an older app without it (the bundle's plugin map answers any name): the next way
        }
      }
      if (!pluginFn(cap, 'Share', 'share')) return base.share(d);
      try {
        await call('Share', 'share', { title: d.title, text: d.text, url: d.url });
        return 'shared';
      } catch {
        return 'cancelled';
      }
    },
    haptic(k) {
      if (pluginFn(cap, 'Haptics', 'impact')) void call('Haptics', 'impact', { style: IMPACT[k] }).catch(() => {});
      else base.haptic(k);
    },
    openExternal,
    // @capacitor/browser: Custom Tabs on Android, SFSafariViewController on iOS; without it, outside
    openInApp(url) {
      if (pluginFn(cap, 'Browser', 'open')) void call('Browser', 'open', { url }).catch(() => openExternal(url));
      else openExternal(url);
    },
    onBack: nativeBack(w),
  };
}
