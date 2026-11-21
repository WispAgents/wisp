// The phone apps' stylesheets (ADR-0065 §5): each look's tokens (@wisp/tokens platform-<look>.css) and,
// from build step 3 on, the kit's variants for it. They are split out of the kit's one stylesheet on
// purpose: the browser never loads them, so its initial CSS stays as it is (ADR-0011). The shell loads
// the page's look once, when it sets `data-platform`; the phone app's own bundle may import
// `@wisp/ui/styles/android` | `ios` up front instead, so its first paint already has them.

import type { Look } from './look.ts';
import { phoneOverlays } from '../controls/phone-part.ts';
import { afterFirstScreen } from '../controls/lazy-part.ts';
import { binders } from '../gestures/binders-part.ts';
import { swipePart } from '../shell-list/swipe-part.ts';
import { loadPhoneStage } from '../app/phone-stage-load.ts';

const LOADERS: Record<Exclude<Look, 'web'>, () => Promise<unknown>> = {
  android: () => import('./android-styles.ts'),
  ios: () => import('./ios-styles.ts'),
};

/** what the page's look loads, once it's asked for (the browser's: nothing) */
const ready: { look: Promise<void> } = { look: Promise.resolve() };

/** Settles once the page's look has its rows' code (at once in the browser): a list waits for it before its first rows,
 *  so a phone's rows are drawn in their Swipe from the start (shell-list/swipe-part.ts). A failed load settles it too
 *  (the rows draw without swipes). */
export const platformReady = (): Promise<void> => ready.look;

/** Loads a phone look's stylesheet and frame, and the overlays' phone halves after the first screen (once; a second
 * call reuses them). The browser's look loads nothing. */
export function loadPlatformStyles(look: Look): Promise<void> {
  if (look === 'web') return Promise.resolve();
  // the overlays' phone halves: no overlay is open on the first screen (ADR-0105), so they come once it's drawn, or
  // with a control's first press (lazy-part.ts), whichever is first
  afterFirstScreen(() => void phoneOverlays.load().catch(() => undefined));
  // the phone frame (app/PhoneStage.tsx), the touch binders (gestures/binders-part.ts) and the row's Swipe come with it
  const rows = Promise.all([binders.load(), swipePart.load()]).then(() => undefined);
  // the list waits only for its rows' code (Swipe, the binders), never the stylesheet's fetch
  ready.look = rows.catch(() => undefined);
  return Promise.all([LOADERS[look](), loadPhoneStage(), rows]).then(() => undefined);
}
