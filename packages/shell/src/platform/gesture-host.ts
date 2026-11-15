// The device's side of the kit's gestures (ADR-0065 build step 7, design/mobile.md §A.5–§A.6): the platform module's
// haptics and native accessibility actions, handed to the kit's `GestureHostProvider` by the shell. Only the phone apps
// get one: the browser and the desktop app keep the kit's no-op host, so a gesture there (there are none: the kit binds
// them under a phone look only) could never buzz or reach the native tree.
import type { GestureHost } from '@wisp/ui';
import { platformLook } from './look.ts';
import type { Platform } from './types.ts';

/** The phone apps' gesture host, or undefined off them (the kit's default host does nothing). */
export function gestureHostOf(p: Pick<Platform, 'kind' | 'haptic' | 'accessibilityActions'>): GestureHost | undefined {
  if (!platformLook(p)) return undefined;
  const actions = p.accessibilityActions;
  return {
    haptic: (kind) => p.haptic(kind),
    ...(actions ? { actions: (el: Element, list: Parameters<typeof actions>[1]) => actions.call(p, el, list) } : {}),
  };
}
