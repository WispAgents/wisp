// The platform the page runs on, for the entry (ADR-0105: the first screen loads what it draws): a browser's page carries
// only the browser's; the desktop app's, Android's and the iPhone's (Tauri's and Capacitor's code) come with an import of
// their own, read from the app's own files there. `detectPlatform` (index.ts) picks the same, synchronously.
import { browserPlatform } from './browser.ts';
import { platformKind } from './kind.ts';
import type { Platform, PlatformWindow } from './types.ts';

export async function loadPlatform(w: PlatformWindow): Promise<Platform> {
  switch (platformKind(w)) {
    case 'desktop':
      return (await import('./desktop.ts')).desktopPlatform(w);
    case 'ios':
      return (await import('./ios.ts')).iosPlatform(w);
    case 'android':
      return (await import('./android.ts')).androidPlatform(w);
    default:
      return browserPlatform(w);
  }
}
