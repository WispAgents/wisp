// wisp.stuff's UI, the part only a press reaches (ADR-0067: PluginUiDef.later): loaded as the first screen draws.
import { lazy } from 'solid-js';
import { entityKey } from '@wisp/client';
import { defineExtension, type PluginUiLater } from '@wisp/shell';
import { taskTabs } from './panes/grant.ts';
import { PANES_KEY } from './pins.tsx';
import { ITEM, type Item } from './types.ts';
import { shareable, type ShareRow } from './share/params.ts';
import './i18n/lazy.ts';

const later: PluginUiLater = {
  // apps pinned beside tasks (v1 panel.js "pane:<id>"): a tab each in the panel beside the open task
  sideTabs: [
    {
      id: 'wisp.stuff.panes',
      tabs: ({ client: c }) => taskTabs(c.entities.row<{ value?: unknown }>('setting', entityKey(c.home, PANES_KEY))()?.value, c.entities.list<Item>(ITEM)() as readonly Item[], (k, v) => c.t(k, v)),
      component: lazy(() => import('./TaskPane.tsx')),
    },
  ],
  extensions: [
    // Share… in a task's row menu (v1 taskMenu: shareable, a task of yours on this computer)
    defineExtension({
      id: 'wisp.stuff.rowShare',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 40,
      if: (p) => shareable(p.session?.() as ShareRow | undefined),
      bar: {
        icon: 'share',
        label: 'stuff.share.menu',
        priority: 40,
        section: 'task',
        select: (p) => p.client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'stuff.share' as never, params: { session: String((p.session?.() as ShareRow | undefined)?.id ?? '') } } }),
      },
    }),
  ],
};
export default later;
