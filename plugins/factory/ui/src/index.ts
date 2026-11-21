// @wisp-plugin/factory-ui: wisp.factory's UI (ADR-0114). The band over a factory run's task (its stations as steps, the
// round of the limit, what waits on you, Open each station's task, Stop) loads with import() when a task page shows, so
// the first screen carries only this index and one string.

import { lazy } from 'solid-js';
import type { ExtensionDef, PluginUiDef } from '@wisp/shell/contract';
import { catalogue } from './catalogue.ts';

const ui: PluginUiDef = {
  id: 'wisp.factory',
  extensions: [{ id: 'wisp.factory.stations', attachTo: { id: 'shell.task:band' }, component: lazy(() => import('./load.ts').then((m) => m.band())) as unknown as ExtensionDef['component'] }],
  catalogue,
};
export default ui;
