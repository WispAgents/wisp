// wisp.worktrees' UI: the Settings → Worktrees pane, and Keep worktree in a task's row menu.
import { lazy } from 'solid-js';
import { defineView, type PluginUiDef } from '@wisp/shell';
import { VIEW } from './view-id.ts';
import { catalogue } from './i18n.ts';
import { doneTitle, parseDone } from './view.ts';

const ui: PluginUiDef = {
  id: 'wisp.worktrees',
  // Done asks about the worktree: after Mark done on this client, the folders it and what ran under it hold
  views: [defineView({ kind: VIEW, slot: 'overlay', overlay: 'dialog', size: 'v1', parse: parseDone, title: doneTitle, component: lazy(() => import('./panes/DoneDialog.tsx')) })],
  // what watches Mark done loads apart from the first page (ADR-0011); what was in flight at start is noted now, so a
  // Done before it lands is still asked about
  setup: (c) => {
    const seen = new Set(c.entities.pending()().map((p) => p.mid));
    void import('./ask.ts').then(
      (m) => m.watchDone(c, seen),
      () => undefined,
    );
  },
  settings: [{ id: 'worktrees', group: 'computers', title: 'worktrees.pane.worktrees', icon: 'branch', order: 40, component: lazy(() => import('./panes/Worktrees.tsx')) }],
  // Keep worktree in a task's row menu: loaded as the first screen draws (ADR-0067)
  later: () => import('./later.ts'),
  catalogue,
};
export default ui;
