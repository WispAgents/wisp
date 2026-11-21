// The phone apps' look (ADR-0065 §5): the shell writes `data-platform="android" | "ios"` on <html> from
// `platform.kind`, so the kit draws Material 3 or HIG. The browser (and the desktop app) never gets it, at
// any width: there, v1's look holds (ADR-0016). This module and packages/ui are the only code that may read
// the platform's look (tools/check `platform-branch`).

import type { Platform, PlatformLook } from './types.ts';

/** The look a platform draws, or undefined for the browser and the desktop app. */
export function platformLook(p: Pick<Platform, 'kind'>): PlatformLook | undefined {
  return p.kind === 'android' || p.kind === 'ios' ? p.kind : undefined;
}

/** Sets (or, off the phone apps, removes) `data-platform` on the root. Returns the look it set. */
export function applyPlatformLook(root: Element, p: Pick<Platform, 'kind'>): PlatformLook | undefined {
  const look = platformLook(p);
  if (look) root.setAttribute('data-platform', look);
  else root.removeAttribute('data-platform');
  return look;
}
