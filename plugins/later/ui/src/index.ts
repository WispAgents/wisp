// @wisp-plugin/later-ui: wisp.later's UI (docs/v2-ui-brief.md). Each extension's component loads with
// import() on first show, so the plugin costs nothing until it's on screen.

import { lazy } from 'solid-js';
import { defineExtension, definePluginUi, defineView } from './define.ts';
import { catalogue } from './i18n.ts';

export default definePluginUi({
  id: 'wisp.later',
  // the follow-ups set for later in a task, over its reply box (v1 later.js followUpItems)
  extensions: [defineExtension({ id: 'wisp.later.foot', attachTo: { id: 'shell.task:composer-top' }, component: lazy(() => import('./ui/LaterFoot.tsx')) })],
  views: [
    // a scheduled task or reminder, or a /draft, opened from its row (shell-list's Scheduled and Drafts)
    defineView<{ id?: string }>({
      kind: 'later.plan',
      slot: 'main',
      parse: (p) => (p && typeof p === 'object' && typeof (p as { id?: unknown }).id === 'string' ? { id: (p as { id: string }).id } : {}),
      component: lazy(() => import('./ui/PlanPage.tsx')),
    }),
  ],
  // the prompt boxes' Send when it means later (a time, /draft, /after): loaded as the first screen draws (ADR-0067)
  later: () => import('./later.ts'),
  commands: [],
  catalogue,
});
