// wisp.worktrees' UI: the Settings → Worktrees pane, and Keep worktree in a task's row menu.
import { lazy } from 'solid-js';
import { watchForDone } from './watch.ts';
import { defineView, type PluginUiDef } from '@wisp/shell';
import { VIEW } from './view-id.ts';
import { catalogue } from './i18n.ts';
import { doneTitle, parseDone } from './view.ts';

const ui: PluginUiDef = {
  id: 'wisp.worktrees',
  // Done asks about the worktree: after Mark done on this client, the folders it and what ran under it hold
  views: [defineView({ kind: VIEW, slot: 'overlay', overlay: 'dialog', size: 'v1', parse: parseDone, title: doneTitle, component: lazy(() => import('./panes/DoneDialog.tsx')) })],
  // Done asks about the worktree: watched from the start, asked by code that loads when one comes (watch.ts)
  setup: watchForDone,
  settings: [{ id: 'worktrees', group: 'computers', keys: () => import('./pane-keys.ts').then((m) => m.PANE_KEYS.worktrees), title: 'worktrees.pane.worktrees', icon: 'branch', order: 40, component: lazy(() => import('./panes/Worktrees.tsx')) }],
  // Keep worktree in a task's row menu: loaded as the first screen draws (ADR-0067)
  later: () => import('./later.ts'),
  catalogue,
};
export default ui;
