// wisp.history's UI, the part only a press reaches (ADR-0067: PluginUiDef.later), loaded as the first screen draws:
// Save to repo in a task's ⋯ menu (any task: `history.track`), and the offer under its last answer (OfferFoot).
import { lazy } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { defineExtension, type PluginUiLater } from '@wisp/shell';
import { historyOf, runSave, saveable, trackCmd, type TaskRow } from './save.ts';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

const later: PluginUiLater = {
  extensions: [
    // under the task's last answer: Save to repo when offered, then Saving… and what was saved
    defineExtension({ id: 'wisp.history.offer', attachTo: { id: 'shell.turn:foot' }, component: lazy(() => import('./OfferFoot.tsx')), order: 40 }),
    defineExtension({
      id: 'wisp.history.rowSave',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 95,
      if: (p) => saveable(p.session?.() as TaskRow | undefined),
      bar: {
        icon: 'branch',
        label: 'history.save',
        priority: 95,
        section: 'keep',
        // saved before: it writes the same files again, with what changed since
        text: (p) => (historyOf(p.session?.() as TaskRow | undefined).tracked ? t('history.saveAgain') : t('history.save')),
        select: (p) => {
          const key = String(p.session?.()?.id ?? '') as EntityKey;
          if (!key) return;
          void runSave((k) => p.client.run(trackCmd, { key: k }), key);
          // the task opens on its answer, which says Saving…, then what was saved (a refusal toasts)
          p.client.navigate({ t: 'openTask', id: key as never });
        },
      },
    }),
  ],
};
export default later;
