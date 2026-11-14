// wisp.grid-client's UI: Settings → Cloud (sign in to a wisp Grid, this computer on it), and the same sign-in in
// Settings → Machines.
import { lazy } from 'solid-js';
import { defineExtension, type PluginUiDef } from '@wisp/shell';
import { catalogue } from './i18n.ts';
import { watchHomeScreen } from './homescreen.ts';
import { previewAddress } from './preview.ts';

const ui: PluginUiDef = {
  id: 'wisp.grid-client',
  settings: [{ id: 'cloud', group: 'computers', title: 'grid-client.pane', icon: 'cloud', order: 20, component: lazy(() => import('./panes/Cloud.tsx')) }],
  // this computer on a grid, in Settings → Machines too, where v1 had it (.mc-grid)
  extensions: [defineExtension({ id: 'grid-client.machines', attachTo: { id: 'shell.settings:machines' }, component: lazy(() => import('./panes/Reach.tsx')) })],
  // a box's localhost in an answer's preview block, at the grid's preview address (ADR-0064)
  previewAddresses: [{ id: 'grid-client.box', map: previewAddress }],
  // on a grid box, the manifest and Apple's Home Screen tags (F-phone-6, v1 homescreen.js)
  setup: watchHomeScreen,
  catalogue,
};
export default ui;
