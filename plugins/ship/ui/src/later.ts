// wisp.ship's UI, the part only a press reaches (ADR-0067: PluginUiDef.later): a task's row menu items, loaded as the
// first screen draws.
import { defineExtension, type PluginUiLater } from '@wisp/shell';
import type { ExtensionProps } from '@wisp/shell/contract';
import { SHIP_VIEW } from './view-id.ts';
import { openPrs, prsOf, shipLabel, shippable } from './model/pr.ts';
import { mergeRequest } from './merge-request.ts';
import { guessShip } from './model/ship.ts';
import { t } from './i18n.ts';
import type { ShipSession } from './types.ts';
import { shipRow } from './row.ts';
import './i18n/lazy.ts';

/** The row an extension is shown for, flat and keyed by its entity key (`p.id` in task inputs). */
const sess = (p: ExtensionProps): ShipSession | undefined => shipRow(p.session?.(), p.id);

const later: PluginUiLater = {
  extensions: [
    // Ship it… in a task's row menu (v1 taskMenu: shippable, its words by the PRs it has), S
    defineExtension({
      id: 'wisp.ship.rowShip',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 50,
      if: (p) => shippable(sess(p)),
      bar: {
        icon: 'send',
        label: 'wisp.ship.action.ship',
        priority: 50,
        section: 'waiting',
        keys: 'S',
        rowOnly: true, // the open task's bar has Ship it
        text: (p) => shipLabel(t, sess(p) as ShipSession),
        select: (p) => {
          const s = sess(p);
          if (s) p.client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: SHIP_VIEW as never, params: { key: s.id, guess: guessShip(s) } as never } });
        },
      },
    }),
    // Merge PR #n… (v1 mergeLabel: its open PRs, none in conflict) and Review PR #n (v1 reviewLabel, P: the PR pane)
    defineExtension({
      id: 'wisp.ship.rowMerge',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 52,
      if: (p) => openPrs(sess(p)).length > 0,
      // the merge mark, not v1's ✓ (it read as done or picked, QA phone r3 #6)
      bar: {
        icon: 'merge',
        label: 'wisp.ship.menu.merge',
        priority: 52,
        section: 'waiting',
        text: (p) => {
          const open = openPrs(sess(p));
          return open.length > 1 ? t('wisp.ship.menu.mergeMany', { count: open.length }) : t('wisp.ship.menu.merge', { n: open[0]?.pr.number ?? 0 });
        },
        select: (p) => {
          const s = sess(p);
          if (!s) return;
          mergeRequest(s.id);
          p.client.navigate({ t: 'openTask', id: s.id as never });
        },
      },
    }),
    defineExtension({
      id: 'wisp.ship.rowReview',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 75, // v1 #786 View ›: Changes, Shell, Review PR, Run app, Watch
      if: (p) => prsOf(sess(p)).length > 0,
      // it looks at the PR; the merge mark is Merge PR's now
      bar: {
        icon: 'eye',
        label: 'wisp.ship.menu.review',
        priority: 75,
        sub: 'view',
        keys: 'P',
        text: (p) => {
          const prs = prsOf(sess(p));
          return prs.length > 1 ? t('wisp.ship.menu.reviewMany') : t('wisp.ship.menu.review', { n: prs[0]?.pr.number ?? 0 });
        },
        select: (p) => {
          const id = sess(p)?.id;
          if (!id) return;
          p.client.navigate({ t: 'openTask', id: id as never });
          p.client.navigate({ t: 'openSide', side: { kind: 'pr', id: id as never } });
        },
      },
    }),
  ],
};
export default later;
