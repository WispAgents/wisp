// The phone apps' stylesheets (ADR-0065 §5): each look's tokens (@wisp/tokens platform-<look>.css) and,
// from build step 3 on, the kit's variants for it. They are split out of the kit's one stylesheet on
// purpose: the browser never loads them, so its initial CSS stays as it is (ADR-0011). The shell loads
// the page's look once, when it sets `data-platform`; the phone app's own bundle may import
// `@wisp/ui/styles/android` | `ios` up front instead, so its first paint already has them.

import type { Look } from './look.ts';
import { phoneOverlays } from '../controls/phone-part.ts';
import { loadPhoneStage } from '../app/phone-stage-load.ts';

const LOADERS: Record<Exclude<Look, 'web'>, () => Promise<unknown>> = {
  android: () => import('./android-styles.ts'),
  ios: () => import('./ios-styles.ts'),
};

/** Loads a phone look's stylesheet and the overlays' phone halves (once; a second call reuses them). The browser's
 * look loads nothing. */
export function loadPlatformStyles(look: Look): Promise<void> {
  if (look === 'web') return Promise.resolve();
  // the overlays' phone halves and the phone frame (app/PhoneStage.tsx) come with the look
  return Promise.all([LOADERS[look](), phoneOverlays.load(), loadPhoneStage()]).then(() => undefined);
}
