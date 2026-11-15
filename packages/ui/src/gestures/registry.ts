// The kit's gesture registry (ADR-0065 build step 7, design/mobile.md §A.4): every gesture the phone apps answer, each
// naming its visible twin, the control on the same screen that does the same thing, so nobody has to know a gesture
// to use wisp. A gesture without a twin doesn't type-check (`defineGestures` requires `twin.command` and at least one
// `twin.parts` entry): test/gestures.test.ts `every_gesture_names_its_control`.
//
// `twin.command` is what the twin runs: a wire command where there is one (`runtime.setState`), else the UI intent the
// twin's button calls (`ui.side-panel`). The phone screens' twins sit in their bar (TopAppBar's actions: Select on
// the list, the side-panel button and the ⋮/⋯ menu with Next task / Previous task on a task page, the me menu with
// Reconnect), so most name `ui.app-bar.actions`. `twin.parts` are the twins' data-part names, written on the gesture's element
// as `data-twin` so qa/platform.mjs (`twinRule`) checks each is on screen in the same view. `action` is the
// accessibility action's name (TalkBack / VoiceOver custom actions, §A.6) when the kit names it; rows' swipe actions
// are pickable (Settings → Swipe actions), so their names come from the row's sides. Pure: no DOM.
import type { KitString } from '../core/i18n.tsx';
import type { Haptic } from './host.tsx';

/** How a gesture moves: which way, and whether a long-press starts it. */
export type GestureKind = 'swipe-x' | 'swipe-left' | 'long-press' | 'pull-down' | 'long-press-drag' | 'drag-down';

export interface GestureTwin {
  /** what the twin runs: a wire command id, or the UI intent its button calls */
  readonly command: string;
  /** the twins' data-part names (at least one), each on screen where the gesture is */
  readonly parts: readonly [string, ...string[]];
}

export interface GestureSpec {
  readonly kind: GestureKind;
  /** what it does, in design/mobile.md §A.4's words */
  readonly does: string;
  readonly twin: GestureTwin;
  /** the accessibility action's name, when the kit names it (else the element's own labels) */
  readonly action?: KitString | readonly KitString[];
  /** the platform module's haptic: `line` on crossing the commit line, `start` when it begins (§A.5) */
  readonly haptic?: { readonly line?: Haptic; readonly start?: Haptic };
}

/** The registry's shape: a gesture must name its twin, or it doesn't compile. */
export function defineGestures<const T extends Readonly<Record<string, GestureSpec>>>(gestures: T): T {
  return gestures;
}

export const GESTURES = defineGestures({
  'row.swipe': {
    kind: 'swipe-x',
    does: 'Mark done / Reopen (right), Save for later / Unsave (left); pickable in Settings → Swipe actions',
    twin: { command: 'runtime.setState', parts: ['ui.app-bar.actions'] },
    haptic: { line: 'select' },
  },
  'row.long-press': {
    kind: 'long-press',
    does: "Android: a bottom sheet of the row's actions with Select first; iOS: the context menu with a preview",
    twin: { command: 'ui.row-menu', parts: ['ui.app-bar.actions'] },
    action: 'gesture.actions',
    haptic: { start: 'tap' },
  },
  'list.pull': {
    kind: 'pull-down',
    does: 'reconnect every computer now (skips the backoff)',
    twin: { command: 'ui.reconnect', parts: ['ui.app-bar.actions'] },
    action: 'gesture.reconnect',
    haptic: { line: 'select' },
  },
  'task.head-swipe': {
    kind: 'swipe-x',
    does: "the next / previous task in the list's order",
    twin: { command: 'ui.next-task', parts: ['ui.app-bar.actions'] },
    action: ['gesture.next-task', 'gesture.previous-task'],
    haptic: { line: 'select' },
  },
  'task.body-swipe': {
    kind: 'swipe-left',
    does: 'the side panel comes in (F-phone-4)',
    twin: { command: 'ui.side-panel', parts: ['ui.app-bar.actions'] },
    action: 'gesture.side-panel',
    haptic: { line: 'select' },
  },
  'queue.drag': {
    kind: 'long-press-drag',
    does: 'reorder the follow-ups',
    twin: { command: 'runtime.queue.order', parts: ['composer.queue.grip'] },
    action: ['gesture.move-up', 'gesture.move-down'],
    haptic: { start: 'tap', line: 'select' },
  },
  // the crew's story on phones (step 13b): a drag down closes it (×), a press held pauses it (Pause); its tap on the left
  // or right half steps through the crew with ← and → on screen
  'story.drag-down': {
    kind: 'drag-down',
    does: "close a crew member's story",
    twin: { command: 'ui.close-story', parts: ['crew.story.close'] },
    action: 'close',
    haptic: { line: 'select' },
  },
  'story.long-press': {
    kind: 'long-press',
    does: 'hold the story still while pressed',
    twin: { command: 'ui.pause-story', parts: ['crew.story.pause'] },
  },
  // My Stuff on phones (step 14): a saved thing's row, Share (right) and Delete (left); both are in its ⋯ menu on screen
  'stuff.swipe': {
    kind: 'swipe-x',
    does: "Share (right) / Delete (left) a saved thing in My Stuff",
    twin: { command: 'stuff.remove', parts: ['stuff.phone.row.menu.trigger'] },
    haptic: { line: 'select' },
  },
  'sheet.drag-down': {
    kind: 'drag-down',
    does: 'close the sheet (a draft stays)',
    twin: { command: 'ui.close-sheet', parts: ['ui.sheet.close'] },
    action: 'close',
    haptic: { line: 'select' },
  },
});

export type GestureName = keyof typeof GESTURES;

/** The markup qa/platform.mjs reads (`data-gesture`, `data-twin`), for the gestures an element answers. */
export function gestureMarks(names: readonly GestureName[]): { readonly 'data-gesture': string; readonly 'data-twin': string } {
  const twins = [...new Set(names.flatMap((n) => GESTURES[n].twin.parts))];
  return { 'data-gesture': names.join(' '), 'data-twin': twins.join(' ') };
}
