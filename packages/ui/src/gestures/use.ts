// `useGesture(name, bind, actions)`: a gesture on the element a ref hands it (ADR-0065 build step 7). Under a phone
// look it marks the element (`data-gesture`, `data-twin`: qa/platform.mjs checks each twin is on screen), binds the
// gesture, and gives the element its accessibility actions through the GestureHost (TalkBack / VoiceOver custom
// actions, design/mobile.md §A.6). In the browser at any width it does nothing at all: no attribute, no listener (v1:
// row swipes are app-only; the browser has the twins it already has), so the browser's HTML stays byte for byte.
import { createEffect, onCleanup } from 'solid-js';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { GESTURES, type GestureName } from './registry.ts';
import { useGestureHost, type GestureAction, type GestureHost } from './host.tsx';
import { useT } from '../core/i18n.tsx';
import { bindPageSwipe } from './bind.ts';
import { bindReorderDrag, bindSheetDrag, type ReorderDragOptions, type SheetDragOptions } from './bind-vertical.ts';

/** Adds `word` to a space-separated attribute (an element may answer several gestures). */
function addWord(el: Element, attr: string, word: string): void {
  const words = (el.getAttribute(attr) ?? '').split(/\s+/).filter(Boolean);
  if (!words.includes(word)) el.setAttribute(attr, [...words, word].join(' '));
}

/** Writes a gesture's marks on its element: its name in `data-gesture`, its twins' parts in `data-twin`. */
export function markGesture(el: Element, name: GestureName): void {
  addWord(el, 'data-gesture', name);
  for (const p of GESTURES[name].twin.parts) addWord(el, 'data-twin', p);
}

/**
 * Call in a component; pass the returned function as the element's `ref`. `bind` starts the gesture on it (one of
 * bind.ts's binders) and returns its stop; `actions` are the same thing for a screen reader, read when bound.
 */
export function useGesture(name: GestureName, bind: (el: HTMLElement, host: GestureHost) => () => void, actions?: () => readonly GestureAction[]): (el: HTMLElement) => void {
  const look = usePlatform();
  const host = useGestureHost();
  return (el) => {
    if (look() === 'web') return;
    markGesture(el, name);
    const stop = bind(el, host);
    onCleanup(stop);
    // the actions follow what's there now (a next task appearing, a row turning disabled)
    const give = host.actions;
    if (actions && give)
      createEffect(() => {
        const list = actions();
        if (!list.length) return;
        const off = give(el, list);
        onCleanup(off);
      });
  };
}

// -------------------------------------------------------------------------------------------------------------------
// The registry's gestures as refs, for the components (and step 8's screens) that carry them.

export interface TaskSwipe {
  /** the next task in the list's order (a swipe left on the head); none: that way only gives */
  readonly onNext?: (() => void) | undefined;
  readonly onPrevious?: (() => void) | undefined;
}

/** A task page's head: swipe left / right for the next / previous task (twins: the page menu's Next / Previous task). */
export function useTaskSwipe(o: () => TaskSwipe): (el: HTMLElement) => void {
  const t = useT();
  return useGesture(
    'task.head-swipe',
    (el, host) => bindPageSwipe(el, { host, can: (d) => (d === 'left' ? o().onNext : o().onPrevious) !== undefined, onSwipe: (d) => (d === 'left' ? o().onNext : o().onPrevious)?.() }),
    () => [
      ...(o().onNext ? [{ label: t('gesture.next-task'), run: () => o().onNext?.() }] : []),
      ...(o().onPrevious ? [{ label: t('gesture.previous-task'), run: () => o().onPrevious?.() }] : []),
    ],
  );
}

/** A task page's body: swipe left (outside the edge zones, not in a scroller) for the side panel (F-phone-4). */
export function useSidePanelSwipe(onPanel: () => void, onDrag?: (dx: number) => void): (el: HTMLElement) => void {
  const t = useT();
  return useGesture(
    'task.body-swipe',
    (el, host) => bindPageSwipe(el, { host, can: (d) => d === 'left', onSwipe: () => onPanel(), ...(onDrag && { onDrag }) }),
    () => [{ label: t('gesture.side-panel'), run: onPanel }],
  );
}

/** A sheet: drag it down to close (a draft stays; twins: Cancel / Close / Done). Step 5's Sheet carries it. */
export function useSheetDrag(o: Omit<SheetDragOptions, 'host'>): (el: HTMLElement) => void {
  const t = useT();
  return useGesture('sheet.drag-down', (el, host) => bindSheetDrag(el, { ...o, host }), () => [{ label: t('close'), run: o.onClose }]);
}

/** A list of follow-ups: long-press an item, then drag it (twins: the grip and Move up / Move down in its menu). */
export function useQueueDrag(o: Omit<ReorderDragOptions, 'host'>): (el: HTMLElement) => void {
  return useGesture('queue.drag', (el, host) => bindReorderDrag(el, { ...o, host }));
}
