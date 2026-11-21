// `usePlatform()`: the look a kit component draws (ADR-0065 §5), for the few whose structure differs by
// platform (design/mobile.md §A.7: a FAB on Android, the compose button in the iOS navigation bar). Kit
// code only: features and plugin UIs never ask (tools/check `platform-branch`). Variants that are only
// colours, sizes and shapes live in CSS under `:root[data-platform=…]` and need no call.
//
// The look comes from <html data-platform>, which the shell sets once before the first render. The
// gallery and tests wrap a subtree in `PlatformLookProvider` to switch it live.

import { createContext, useContext, type Accessor, type JSX } from 'solid-js';
import { documentLook, type Look } from './look.ts';

const LookContext = createContext<Accessor<Look>>();

// one attribute read: the shell sets it before the first render and the apps never change it
const documentAccessor: Accessor<Look> = () => documentLook(typeof document === 'undefined' ? undefined : document);

/** The look to draw: the nearest `PlatformLookProvider`'s, else the page's `data-platform` ('web' without it). */
export function usePlatform(): Accessor<Look> {
  return useContext(LookContext) ?? documentAccessor;
}

export function PlatformLookProvider(props: { readonly look: Accessor<Look>; readonly children: JSX.Element }): JSX.Element {
  return <LookContext.Provider value={props.look}>{props.children}</LookContext.Provider>;
}
