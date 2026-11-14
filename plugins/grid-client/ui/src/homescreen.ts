// F-phone-6 on a Grid only (v1 homescreen.js): once this wisp's row says it runs in a grid box (its preview template,
// ADR-0064), the page gets the manifest and Apple's Home Screen tags, so iPhone Safari's Add to Home Screen opens it
// full screen. Anywhere else (someone's own computer, the native apps) the page never offers to install wisp.
import { subscribe, type PluginClient } from '@wisp/client';
import { addHomeScreenTags, homeScreenTags, nativeShell } from '@wisp/shell/pure';
import type { CloudRow } from './commands.ts';
import { cloudRow } from './preview.ts';

export function watchHomeScreen(c: PluginClient): void {
  if (typeof document === 'undefined' || typeof window === 'undefined') return;
  const native = nativeShell(window as Parameters<typeof nativeShell>[0]);
  if (native) return;
  const row = cloudRow(c);
  // a box is told so in its env (WISP_BOX, WISP_PREVIEW_URL): the first row that lands decides for this page
  const decide = (r: CloudRow | undefined): boolean => {
    if (!r) return false;
    addHomeScreenTags(document, homeScreenTags({ inBox: Boolean(r.preview), native }));
    return true;
  };
  if (decide(row())) return;
  const stop = subscribe(row, (r) => {
    if (decide(r)) stop();
  });
  c.scope.add(stop);
}
