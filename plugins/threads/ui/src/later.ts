// wisp.threads' UI, the part only a press reaches (ADR-0067: PluginUiDef.later), loaded as the first screen draws:
// the Thread tab beside a task (v1 panel.js tab 'thread', after the panel's own), and Thread in the task's menu (on
// a phone, where the side panel isn't beside the task: it opens the Thread as a sheet).
import { lazy } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { defineExtension, type PluginUiLater } from '@wisp/shell';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { threadable, type TaskLike } from './model.ts';
import { KIND, THREAD_VIEW, type HealthRow } from './types.ts';

const later: PluginUiLater = {
  sideTabs: [
    {
      id: 'wisp.threads.tab',
      order: 10,
      // a task with an agent of its own (v1 threadable); its tooltip says when the conversation is worn
      tabs: ({ client, task }) => {
        if (!threadable(client.entities.row<TaskLike>(KIND.session, task as EntityKey)() as TaskLike | undefined)) return [];
        const level = client.entities.row<HealthRow>(KIND.health, task as EntityKey)()?.body.level;
        // the task's own conversation, read in the page already: behind the panel's + (UX 2026-11-18 #5)
        return [{ value: 'thread', label: t('threads.tab'), title: t(level === 'worn' ? 'threads.tab.tipWorn' : 'threads.tab.tip'), icon: 'transcript', quiet: true }];
      },
      component: lazy(() => import('./ThreadTab.tsx')),
    },
  ],
  extensions: [
    // under a compaction's turn: what it kept of the Notes (ADR-0096; only a turn that has an audit shows anything)
    defineExtension({ id: 'threads.audit', attachTo: { id: 'shell.turn:foot' }, component: lazy(() => import('./AuditFoot.tsx')), order: 30 }),
    defineExtension({
      id: 'wisp.threads.menu',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 40,
      if: (p) => threadable(p.session?.() as TaskLike | undefined),
      bar: {
        icon: 'transcript',
        label: 'threads.title',
        priority: 40,
        section: 'look',
        select: (p) => {
          const s = p.session?.();
          if (s) p.client.navigate({ t: 'openOverlay', overlay: { kind: 'sheet', id: THREAD_VIEW as never, params: { task: s.id } } });
        },
      },
    }),
  ],
};

export default later;
