// @wisp-plugin/ship-ui: wisp.ship's UI (v1 ship.js). A task row's PR pill, the PR strip over the open task's
// turns, and Ship it's dialog (an overlay view). Each component loads on first show.
import { lazy } from 'solid-js';
import { defineExtension, definePluginUi, defineView } from '@wisp/shell';
import { catalogue } from './i18n.ts';
import { parseShipParams } from './params.ts';
import { SHIP_VIEW } from './view-id.ts';
import { prOf } from './model/prs.ts';
import { dialogTitle } from './model/title.ts';
import { t } from './i18n.ts';
import type { ShipSession } from './types.ts';
import { shipRow } from './row.ts';
import type { ExtensionProps } from '@wisp/shell/contract';

/** The row an extension is shown for, flat and keyed by its entity key (`p.id` in task inputs). */
const sess = (p: ExtensionProps): ShipSession | undefined => shipRow(p.session?.(), p.id);

export default definePluginUi({
  id: 'wisp.ship',
  catalogue: catalogue as Record<string, string>,
  // Ship it…, Merge PR and Review PR in a task's row menu: loaded as the first screen draws (ADR-0067)
  later: () => import('./later.ts'),
  extensions: [
    // `● #412` on a task's row, in the PR's tone
    // only on a row with a PR or a ship in flight: the rest get no pill's box at all (v1)
    defineExtension({
      id: 'wisp.ship.pill',
      attachTo: { id: 'shell.row:head' },
      component: lazy(() => import('./PrPill.tsx')),
      if: (p) => {
        const s = sess(p);
        return !!s?.shipping || prOf(s) !== undefined;
      },
    }),
    // the PR row over the open task: its line and what to do next (Merge, Fix conflicts)
    defineExtension({ id: 'wisp.ship.strip', attachTo: { id: 'shell.task:band' }, component: lazy(() => import('./PrStrip.tsx')) }),
  ],
  // its title from the click's guess ("Ship it", "Push to PR #412"), never the shell's fallback
  views: [defineView({ kind: SHIP_VIEW, slot: 'overlay', overlay: 'dialog', parse: parseShipParams, title: (p) => dialogTitle(t, p.guess), component: lazy(() => import('./ShipDialog.tsx')) })],
});
