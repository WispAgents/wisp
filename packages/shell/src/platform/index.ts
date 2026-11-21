// Picks the platform the page runs on: Tauri's internals mean the desktop app, a native Capacitor
// bridge the Android or the iPhone app (Capacitor's getPlatform says which), anything else the browser. Features take a `Platform`; nothing else in
// the UI asks which one it is.

import { androidPlatform } from './android.ts';
import { browserPlatform } from './browser.ts';
import { desktopPlatform } from './desktop.ts';
import { iosPlatform } from './ios.ts';
import { platformKind } from './kind.ts';
import type { Platform, PlatformWindow } from './types.ts';

export function detectPlatform(w: PlatformWindow): Platform {
  const kind = platformKind(w);
  if (kind === 'desktop') return desktopPlatform(w);
  if (kind === 'ios') return iosPlatform(w);
  if (kind === 'android') return androidPlatform(w);
  return browserPlatform(w);
}

/** For tests and server-side checks: every call answers 'unsupported' and nothing happens. */
export function nullPlatform(over: Partial<Platform> = {}): Platform {
  return {
    kind: 'browser',
    mobile: false,
    phone: false,
    standalone: false,
    apple: true,
    titlebar: 'none',
    origin: 'http://localhost',
    dragRegions: () => () => {},
    notifyPermission: async () => 'unsupported',
    requestNotify: async () => 'unsupported',
    notify: async () => 'unsupported',
    badge: async () => 'unsupported',
    share: async () => 'unsupported',
    pickFiles: async () => [],
    haptic: () => {},
    copy: async () => false,
    openExternal: () => {},
    onResume: () => () => {},
    onOpen: () => () => {},
    reload: () => {},
    safeArea: () => ({ top: 0, right: 0, bottom: 0, left: 0 }),
    windowLayout: () => ({ width: 1440, height: 900, size: 'expanded' }),
    openInApp: () => {},
    onBack: () => () => {},
    ...over,
  };
}

export { androidPlatform, browserPlatform, desktopPlatform, iosPlatform };
export { applyPlatformLook, platformLook } from './look.ts';
export { nativeSeams } from './native-seams.ts';
export { sizeClass } from './web.ts';
export { loadPlatform } from './load.ts';
export type * from './types.ts';
