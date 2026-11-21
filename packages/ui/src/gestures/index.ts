// The phone apps' gestures and their visible twins (ADR-0065 build step 7, design/mobile.md §A.4–§A.6).
export { GESTURES, defineGestures, gestureMarks, type GestureKind, type GestureName, type GestureSpec, type GestureTwin } from './registry.ts';
export { GestureHostProvider, useGestureHost, type GestureAction, type GestureHost, type Haptic } from './host.tsx';
export { bindLongPress, bindPageSwipe, inScroller, ownerOf, OWN_TOUCH, type Dir as GestureDir, type LongPressOptions, type PageSwipeOptions } from './bind.ts';
export { bindPull, bindReorderDrag, bindSheetDrag, type PullOptions, type ReorderDragOptions, type SheetDragOptions } from './bind-vertical.ts';
export { pressTimers, type Schedule as PressSchedule } from './press-scope.ts';
export { markGesture, useGesture, useQueueDrag, useSheetDrag, useSidePanelSwipe, useTaskSwipe, type TaskSwipe } from './use.ts';
export * from './gesture-math.ts';
