// `@wisp/ui/pure`: the kit's plain functions, without the kit. The main entry pulls in every component (seconds to
// compile in a test), so a plugin's plain module imports these from here and its tests stay fast. Only modules
// that reach no component belong here.
// the bridge from @wisp/client's signals into Solid (no component): plain modules that derive over the store use it
export { useDerived, useSignal } from './core/signal.ts';
export { reorderable, reordered, type OnReorder, type ReorderAttrs } from './core/reorder.ts';
export { menuSections, type MenuEntry, type MenuGroup, type MenuItem, type MenuSeparator, type MenuSubmenu } from './controls/menu-entries.ts';
export type { IconName } from './content/icons.ts';
// the composer's `+` (the shell's composer-plus.ts) builds a tool from acts: types only, nothing of act.tsx loads
export type { Act } from './app/act.tsx';
export type { PartName } from './core/props.ts';
export { isSpirit, parseSpirit, spiritCode, type SpiritPart, type SpiritParts } from './home/spirit-shape.ts';
export { themeCss, type FrameScheme } from './frame/theme.ts';
export { LOOK_ATTR, documentLook, lookOf, type Look } from './platform/look.ts';
export { loadPlatformStyles } from './platform/styles.ts';
export { FAB_SLOP, FAB_START, barCollapsed, fabOnScroll, type FabScroll } from './nav/scroll-math.ts';
export { isEnvFile, maskEdit } from './content/env-mask.ts';
export type { DiffFile, DiffHunk, DiffLine, DiffStatus } from './content/diff/parse.ts';
export { makerOptions, type MakerOption } from './crew/maker-options.ts';
export { GESTURES, defineGestures, gestureMarks, type GestureKind, type GestureName, type GestureSpec, type GestureTwin } from './gestures/registry.ts';
export * from './gestures/gesture-math.ts';
// the task history as a ledger (ADR-0068): days, notice folds, search and filters, the scrubber's ticks, the keys
export * from './app/ledger-model.ts';
export { foldTail, tailFold, type FoldExtras, type FoldInput, type LauncherTool } from './app/launcher-fold.ts';
// work that waits for the first screen (ADR-0105): plain plugin modules (a watcher started at setup) queue on it
export { afterFirstScreen } from './controls/lazy-part.ts';
