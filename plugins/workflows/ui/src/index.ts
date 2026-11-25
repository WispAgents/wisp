// @wisp-plugin/workflows-ui: wisp.workflows' UI (ADR-0119). The band over a task with workflow runs (the live board:
// phases, steps, fan-outs, Stop) loads with import() when a task page shows; `/workflow` in a prompt box loads as the
// first screen draws (`later`), so the first screen carries only this index and its few strings.

import { lazy } from 'solid-js';
import type { ExtensionDef, PluginUiDef } from '@wisp/shell/contract';
import { catalogue } from './catalogue.ts';

const ui: PluginUiDef = {
  id: 'wisp.workflows',
  later: () => import('./later.ts'),
  extensions: [{ id: 'wisp.workflows.board', attachTo: { id: 'shell.task:band' }, component: lazy(() => import('./load.ts').then((m) => m.band())) as unknown as ExtensionDef['component'] }],
  catalogue,
};
export default ui;
