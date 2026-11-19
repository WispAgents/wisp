// The row's Swipe (Swipe.tsx), loaded apart (ADR-0105): only the phone apps swipe rows, so the browser never fetches it;
// the phone apps load it with their look (platform/styles.ts), and a list waits for that (`platformReady`) before its
// first rows, so a row is drawn inside its Swipe from the start (never drawn bare, then again wrapped).
import { createComponent, createMemo, untrack, type JSX } from 'solid-js';
import { lazyPart } from '../controls/lazy-part.ts';
import type { SwipeProps } from './Swipe.tsx';

export const swipePart = lazyPart(() => import('./Swipe.tsx'), { preload: false });

/** Swipe once it's loaded, the row alone until then (the browser: always). It never asks for Swipe itself: the phone
 *  app's look does, before the list draws, so a failed load leaves rows bare rather than rebuilding them later. */
export function LazySwipe(props: SwipeProps): JSX.Element {
  return createMemo(() => {
    const m = swipePart.ready();
    return untrack(() => (m ? createComponent(m.Swipe, props) : props.children));
  }) as unknown as JSX.Element;
}
