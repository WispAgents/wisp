// @wisp-plugin/workflows-ui: wisp.workflows' UI (ADR-0119, ADR-0120). The workflow row in the open task's details
// column (first among the plugins' rows, over the PR card: a workflow working in the task is in sight however far it
// scrolls) and the board it opens in the main area load with import() when a task page shows them; `/workflow` in a
// prompt box loads as the first screen draws (`later`), so the first screen carries only this index and its strings.

import { lazy } from 'solid-js';
import { defineExtension, definePluginUi, defineView } from './define.ts';
import { catalogue } from './catalogue.ts';
import { VIEW, parseBoard } from './views.ts';

export default definePluginUi({
  id: 'wisp.workflows',
  later: () => import('./later.ts'),
  extensions: [defineExtension({ id: 'wisp.workflows.card', attachTo: { id: 'shell.task:details' }, order: -100, component: lazy(() => import('./load.ts').then((m) => m.card())) })],
  views: [
    defineView({
      kind: VIEW.board,
      slot: 'main',
      parse: parseBoard,
      component: lazy(() => import('./load.ts').then((m) => m.board())) as never,
      title: () => catalogue['wisp.workflows.boardTitle'] ?? 'Workflow',
    }),
  ],
  catalogue,
});
