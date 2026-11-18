// `@wisp/shell/pure`: the shell's plain functions, without the shell. The main entry pulls in every frame
// component (seconds to compile in a test), so a plugin's plain module (its rules, its list model) imports these
// from here and its tests stay fast. Only modules that reach no component belong here.
export { editorUrl } from './editor.ts';
export { clock24, cronWords, midSentence, toCron, type ClockFormat } from './schedule.ts';
export { PROJECT_ORDER, movedOrder, projectList, projectOf, projectOfRow, type ProjectView } from './frame/rows-view.ts';
export { dirsOf, launchOf, parsePick, pickValue, placePick, taskPlaces, type TaskPlace } from './frame/project-pick.ts';
// New task's command (its optimistic row, its wire): the dev mock's tests send it as New task does
export { create, startingRow, type CreateArgs } from './frame/commands.ts';
export { menuEntryOf, taskMenu, type TaskMenuEntry, type TaskMenuInput } from './task-menu.ts';
export { effortWord, modelWords, runLabel } from './frame/run-words.ts';
export { STANDALONE_CHECK, HOME_SCREEN_TAGS, homeScreenTags, addHomeScreenTags, nativeShell, type HomeScreenTag } from './platform/standalone.ts';
// a task's turn rows as the history ledger reads them (ADR-0068)
export { NOTICE_KINDS, clause, isNotice, ledgerTurn, outcomeOf, shippedOf, type TurnLike } from './frame/ledger-turns.ts';
export { COMPUTERS_VIEW, destinationOf, destinationsOf, intentOf, type DestinationId } from './frame/destinations.ts';
// a task held before it starts: the list, Home and the task header say Held by this one rule
export { isHeld } from './task-held.ts';
// Auto's one switch (`runtime.auto`, default off: owner, 2026-11-17): plugins that show Auto ask it
export { AUTO_SETTING, autoEnabled, autoOn } from './frame/launch-pick.ts';
