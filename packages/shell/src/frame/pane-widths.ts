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

/** v1 panel.js PANEL_WIDE: narrower than this, the side panel takes the list's place while it's open */
export const PANEL_WIDE = 1180;

/** Whether the list shows beside the open task: as you left it, unless the side panel is open in a window too narrow
 *  for both (v1 listHidden: sideHidden || panelSqueeze). Your choice is kept: it comes back as the panel closes. */
export function listShown(open: boolean, sideOpen: boolean, win: number): boolean {
  return open && !(sideOpen && win < PANEL_WIDE);
}

/** The list's and the panel's widths beside the open task (QA round 3 #2). The panel closed: the list as dragged
 *  (`listWidth`). The panel open: the list as dragged, but never so wide that the panel's minimum and the task's 480
 *  don't fit; the panel takes what it leaves (v1 paintWidth: the panel gives way), down to its minimum. */
export function besideTask(o: { readonly win: number; readonly rail: number; readonly list: number; readonly panel: number; readonly sideOpen: boolean }): { list: number; panel: number } {
  const want = listWidth(o.list, o.win, o.rail);
  const list = o.sideOpen ? Math.round(Math.max(SIDE_MIN, Math.min(want, o.win - o.rail - PANEL_MIN - TASK_MIN))) : want;
  return { list, panel: panelWidth(o.panel, o.win, o.rail + list) };
}

/** Dragging the list with the panel open (`panel`: its width now): it stops where the panel and the task's 480 begin,
 *  so widening it takes room from the transcript's spare only, never the panel's or below the task's minimum. */
export function listDrag(o: { readonly win: number; readonly rail: number; readonly want: number; readonly panel: number }): number {
  return Math.round(Math.max(SIDE_MIN, Math.min(listWidth(o.want, o.win, o.rail), o.win - o.rail - o.panel - TASK_MIN)));
}
