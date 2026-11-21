// The Capacitor iPhone app (mobile/ios, ADR-0065 build step 10). Its Swift side is glue only; until it
// grows its own plugins, everything here is a stock Capacitor plugin or the WKWebView's own browser API.

import { browserPlatform } from './browser.ts';
import { capacitorPlatform } from './capacitor.ts';
import type { Platform, PlatformWindow } from './types.ts';

export function iosPlatform(w: PlatformWindow): Platform {
  const base = browserPlatform(w);
  return { ...capacitorPlatform(w, 'ios', (url) => base.openExternal(url)), apple: true };
}
