// Whether the side panel shows its place row (where the task's work is, its branch, what isn't committed) over a pane.
// v1 panel.js: `.side-panel.on-watch .pn-place { display: none }` for Watch, the browser (Preview) and an app pinned
// beside tasks: those panes are a page of their own, not the task's folder.
import type { SidePane } from '../open-task.ts';

export const showsPlace = (pane: SidePane): boolean => pane !== 'watch' && pane !== 'preview' && pane !== 'tab';
