// wisp.telemetry's UI: Settings → Privacy (usage stats, and the repo-trust question), and Home's feedback line.
import { lazy } from 'solid-js';
import { drawnLater } from '@wisp/ui';
import { defineExtension, type PluginUiDef } from '@wisp/shell';
import { catalogue } from './i18n.ts';

const ui: PluginUiDef = {
  id: 'wisp.telemetry',
  settings: [{ id: 'privacy', group: 'advanced', title: 'telemetry.privacy.pane', icon: 'info', order: 30, component: lazy(() => import('./panes/Privacy.tsx')) }],
  // Home's last line, "Hit a bug… Tell us", while feedback can be sent (v1 split.js .sh-fb)
  extensions: [defineExtension({ id: 'telemetry.homeFeedback', attachTo: { id: 'shell.home:feedback' }, component: drawnLater(lazy(() => import('./HomeFeedback.tsx'))) })],
  catalogue,
};
export default ui;
