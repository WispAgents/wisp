// The shell's own settings (its plugin's declarations, plan §6.2), apart from the frame so a test reads them without
// building it. `server` keys are rows on the peer and need a manifest that declares them (wisp-kernel settings.rs
// refuses a write to any other: "no plugin declares the setting"); `device` keys live on this client.

import type { Json } from '@wisp/client';
import { THEMES } from './theme.ts';

/** v1 split.js SIDE_W and clampW's bounds */
export const SIDE_W = 340;
export const SIDE_MIN = 220;
export const SIDE_MAX = 720;
/** v1 panel.js PANEL_W and MIN_W; the most a drag can ask (the window clamps it further) */
export const PANEL_W = 480;
export const PANEL_MIN = 360;
export const PANEL_MAX = 1600;

export const SHELL_SETTINGS = [
  /** light, dark or system: this device's, as the server keeps it (crates/wisp/src/settings_store.rs). No manifest
   *  declares a `shell.*` key, so as a server key every switch was refused and the theme never changed. */
  { key: 'shell.theme', type: 'enum', values: THEMES, default: 'system', scope: 'device', overridable: true },
  /** the task list's width, dragged on its grip (v1 wisp.sideW: 340 by default, 220 to 720, kept on this device) */
  { key: 'shell.sidebar.width', type: 'int', min: SIDE_MIN, max: SIDE_MAX, default: SIDE_W, scope: 'device' },
  /** the task list shown beside the task, or hidden by the sidebar button or [ (v1 wisp.sideHidden) */
  { key: 'shell.sidebar.open', type: 'bool', default: true, scope: 'device' },
  /** the side panel's width (Changes, PR, Shell), dragged on its inner edge (v1 panel.js pref.w: 480, at least 360) */
  { key: 'shell.panel.width', type: 'int', min: PANEL_MIN, max: PANEL_MAX, default: PANEL_W, scope: 'device' },
  /** the room Home's usage row took last time (px), held while it loads so Needs you doesn't move (QA round 1 #13) */
  // the default is the row's one line (10px above it, 33 tall, 8 drawn back: app-home.css), so a first load doesn't
  // jump either; a load where no row shows lets it go (Reserved reports 0)
  { key: 'shell.home.usageHeight', type: 'int', min: 0, max: 400, default: 35, scope: 'device' },
  /** the room Home's cards under Needs you took last time (wisp.budget's Spare allowance), held while they load */
  { key: 'shell.home.cardsHeight', type: 'int', min: 0, max: 800, default: 0, scope: 'device' },
  /** the project the list shows (the top bar's chips): a project's id, or 'all' (v1 wisp.filter) */
  { key: 'shell.projectFilter', type: 'string', default: 'all', scope: 'device' },
  /** the phone apps' computer filter (the Tasks bar's title): a peer id, or 'all' (ADR-0065 step 13) */
  { key: 'shell.computerFilter', type: 'string', default: 'all', scope: 'device' },
  /** other wisps this device connects to: [{ peer, name, base }] */
  { key: 'shell.peers', type: 'json', default: [] as Json[], scope: 'device' },
] as const;
