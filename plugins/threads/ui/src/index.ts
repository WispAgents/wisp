// @wisp-plugin/threads-ui: wisp.threads' UI (ADR-0079): a task's Thread, its Notes section by section and how worn
// its conversation is, in a tab beside the task and from the task's menu (a sheet on a phone). Nothing of it is on
// the first page: the tab and the menu entry come with `later`, the Thread itself with import() on first show.
import { lazy } from 'solid-js';
import { definePluginUi, defineView } from '@wisp/shell';
import { catalogue, t } from './i18n.ts';
import { THREAD_VIEW } from './types.ts';

export default definePluginUi({
  id: 'wisp.threads',
  catalogue,
  later: () => import('./later.ts'),
  views: [
    defineView<{ task: string }>({
      kind: THREAD_VIEW,
      slot: 'overlay',
      overlay: 'sheet',
      parse: (p) => {
        const task = (p as { task?: unknown } | null)?.task;
        if (typeof task !== 'string' || task === '') throw new Error('a Thread names its task');
        return { task };
      },
      title: () => t('threads.title'),
      component: lazy(() => import('./ThreadSheet.tsx')),
    }),
  ],
});
