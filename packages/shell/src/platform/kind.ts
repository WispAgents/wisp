// Which platform the page runs on, apart from the platforms themselves: Tauri's internals mean the desktop app, a native
// Capacitor bridge the Android or the iPhone app (Capacitor's getPlatform says which), anything else the browser.
import type { PlatformWindow } from './types.ts';

export type PlatformKind = 'desktop' | 'ios' | 'android' | 'browser';

export function platformKind(w: PlatformWindow): PlatformKind {
  if (w.__TAURI_INTERNALS__) return 'desktop';
  if (w.Capacitor?.isNativePlatform?.() === true && w.Capacitor.getPlatform?.() === 'ios') return 'ios';
  // the app's WebView on a computer's own page has MainActivity's bridge even without Capacitor's
  if (w.Capacitor?.isNativePlatform?.() === true || w.wispAndroid !== undefined) return 'android';
  return 'browser';
}
