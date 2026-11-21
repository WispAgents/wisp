// What a gesture needs from the device (design/mobile.md §A.5, §A.6), handed to the kit by the shell: the platform
// module's `haptic` (packages/shell/src/platform/types.ts `Platform.haptic`: `select` on crossing a line, `tap` on a
// long-press opening a menu; off with the system's setting, which the platform module honours) and the native
// accessibility actions (TalkBack `AccessibilityNodeInfo` custom actions, VoiceOver `UIAccessibilityCustomAction`),
// so a screen reader runs every gesture without making it. The kit can't import the shell (layers), so the shell
// mounts `GestureHostProvider` with its platform module; without one (the browser, tests) both are no-ops and the
// actions stay only in the DOM (`aria-actions` when the element names its twins' ids, plus the twins themselves).
import { createContext, useContext, type JSX } from 'solid-js';

/** The platform module's haptics (the same names as `Haptic` in packages/shell/src/platform/types.ts). */
export type Haptic = 'tap' | 'select' | 'success' | 'warning' | 'error';

/** A gesture as a screen reader's action: its name in the menus' words and what it runs. */
export interface GestureAction {
  readonly label: string;
  readonly run: () => void;
}

export interface GestureHost {
  haptic(kind: Haptic): void;
  /** Gives `el` these custom accessibility actions in the native tree; returns the function that takes them off. */
  actions?(el: Element, actions: readonly GestureAction[]): () => void;
}

const NONE: GestureHost = { haptic: () => undefined };
const HostContext = createContext<GestureHost>(NONE);

/** The device's side of the gestures for every kit component below it (the shell passes its platform module). */
export function GestureHostProvider(props: { readonly host: GestureHost; readonly children?: JSX.Element }): JSX.Element {
  return <HostContext.Provider value={props.host}>{props.children}</HostContext.Provider>;
}

export function useGestureHost(): GestureHost {
  return useContext(HostContext);
}
