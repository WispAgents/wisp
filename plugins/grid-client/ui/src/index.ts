// wisp.grid-client's UI: Settings → Cloud (sign in to a wisp Grid, this computer on it), and the same sign-in in
// Settings → Machines.
import { lazy } from 'solid-js';
import { defineExtension, type PluginUiDef } from '@wisp/shell';
import { catalogue, t } from './i18n.ts';
import { gridHome, offerReach } from './back.ts';
import { watchHomeScreen } from './homescreen.ts';
import { cloudRow, previewAddress } from './preview.ts';
import { PANE_KEYS } from './pane-keys.ts';

const ui: PluginUiDef = {
  id: 'wisp.grid-client',
  settings: [{ id: 'cloud', group: 'computers', keys: PANE_KEYS.cloud, title: 'grid-client.pane', icon: 'cloud', order: 20, component: lazy(() => import('./panes/Cloud.tsx')) }],
  // this computer on a grid, in Settings → Machines too, where v1 had it (.mc-grid)
  extensions: [defineExtension({ id: 'grid-client.machines', attachTo: { id: 'shell.settings:machines' }, component: lazy(() => import('./panes/Reach.tsx')) })],
  // a box's localhost in an answer's preview block, at the grid's preview address (ADR-0064)
  previewAddresses: [{ id: 'grid-client.box', map: previewAddress }],
  // a cloud wisp's way back to its grid's Home (docs/ux/2026-11-18-grid.md #9), in the account menu
  accountLinks: [{ id: 'grid-client.home', label: () => t('grid-client.back.grid'), hint: () => t('grid-client.back.gridHint'), icon: 'cloud', href: (c) => gridHome(cloudRow(c)()) }],
  // putting this computer on a grid, among Home's set-up tips: never on a grid box, which is on one already
  homeTips: [
    {
      id: 'grid-client.reach',
      icon: 'cloud',
      title: () => t('grid-client.tip.reach'),
      body: () => t('grid-client.tip.reachBody'),
      cta: () => t('grid-client.tip.setUp'),
      offered: (c) => offerReach(cloudRow(c)()),
      open: ({ settings }) => settings('cloud'),
    },
  ],
  // on a grid box, the manifest and Apple's Home Screen tags (F-phone-6, v1 homescreen.js)
  setup: watchHomeScreen,
  catalogue,
};
export default ui;
