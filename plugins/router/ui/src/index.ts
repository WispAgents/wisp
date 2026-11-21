// @wisp-plugin/router-ui: wisp.router's UI (docs/v2-ui-brief.md). Each extension's component loads with
// import() on first show, so the plugin costs nothing until it's on screen.

import { lazy } from 'solid-js';
import { autoOn } from '@wisp/shell/pure';
import { definePluginUi } from './define.ts';
import { catalogue } from './i18n.ts';

export default definePluginUi({
  id: 'wisp.router',
  // the scaffold's probe (a row count, Probe.tsx) isn't attached: it showed users a debug pill over the list
  extensions: [],
  views: [],
  // Settings → Auto (v1 settings.js data-pane="auto"): the goal, learning, harnesses, What it learned
  // shown only while Auto's one switch is on (`runtime.auto`, off until the owner reworks it: 2026-11-17)
  settings: [{ id: 'auto', group: 'agents', title: 'wisp.router.pane.title', icon: 'spark', order: 20, component: lazy(() => import('./AutoPane.tsx')), if: (c) => autoOn(c) }],
  commands: [],
  catalogue,
});
