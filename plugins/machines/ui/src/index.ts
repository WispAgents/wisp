// wisp.machines' UI: Settings → Machines (other computers over SSH, and this one reached over SSH), opening the Machines
// dialog (also in the avatar's menu, where v1 kept #machinesBtn: never in the list's head); and the cloud computers in
// Settings → Cloud.
import { lazy } from 'solid-js';
import { defineExtension, type PluginUiDef } from '@wisp/shell';
import { catalogue } from './i18n.ts';
import { MACHINES_VIEW } from './view-id.ts';

const ui: PluginUiDef = {
  id: 'wisp.machines',
  views: [
    {
      kind: MACHINES_VIEW,
      slot: 'overlay',
      overlay: 'dialog',
      size: 'lg',
      part: 'machines.manage.dialog',
      parse: () => ({}),
      component: lazy(() => import('./MachinesView.tsx')) as never,
      title: () => catalogue['machines.title'] ?? 'Machines',
    },
  ],
  settings: [{ id: 'machines', group: 'computers', title: 'machines.pane', icon: 'server', order: 10, component: lazy(() => import('./panes/Machines.tsx')) }],
  extensions: [
    // Settings → Cloud's cloud computers (v1 .cg-list), under the account
    defineExtension({ id: 'machines.cloud', attachTo: { id: 'shell.settings:cloud' }, component: lazy(() => import('./panes/CloudComputers.tsx')) }),
  ],
  catalogue,
};
export default ui;
