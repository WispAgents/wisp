// The task list's and the side panel's widths beside the open task (v1 split.js clampW, panel.js paintWidth): what
// you dragged, kept within what the window leaves. Pure: the frame passes the window's width and the rail's.

import { PANEL_MAX, PANEL_MIN, SIDE_MAX, SIDE_MIN } from '../settings-decls.ts';

/** v1 panel.js TASK_MIN: the task beside the panel never gets narrower */
export const TASK_MIN = 480;
/** v1 clampW: the task keeps 420px beside the list, the list at least 260 however narrow the window */
const LIST_ROOM = 420;
const LIST_FLOOR = 260;

/** The list's width: 220 to 720, and no more than the window leaves the task (v1 clampW). */
export function listWidth(want: number, win: number, rail: number): number {
  const most = Math.max(LIST_FLOOR, Math.min(SIDE_MAX, win - LIST_ROOM - rail));
  return Math.round(Math.min(Math.max(want, SIDE_MIN), most));
}

/** The side panel's width: at least 360, at most 1600, and never so wide the task beside it is under 480 (v1 paintWidth). `list` is
 *  the list's column (its width and the rail), 0 while it's hidden. */
export function panelWidth(want: number, win: number, list: number): number {
  // and never past what the setting keeps (a wider value is refused, and the panel would snap back)
  const most = Math.max(PANEL_MIN, Math.min(PANEL_MAX, win - list - TASK_MIN));
  return Math.round(Math.min(Math.max(want, PANEL_MIN), most));
}
