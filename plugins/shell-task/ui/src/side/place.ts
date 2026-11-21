// Whether the side panel shows its place row (where the task's work is, its branch, what isn't committed) over a pane.
// v1 panel.js showed it over Changes, PR and Shell. UX 2026-11-18 desktop #5: "This computer" repeated under every tab,
// so the row shows only over Shell, and only for a task on another computer (where the shell runs is news then). The
// row's ⋯ lives in the panel's head, over every pane.
import type { SidePane } from '../open-task.ts';

export const showsPlace = (pane: SidePane, remote: boolean): boolean => remote && pane === 'shell';
