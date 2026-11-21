// wisp.stuff's UI: My Stuff (what you and your agents kept), opened from the sidebar.
import { lazy } from 'solid-js';
import { defineExtension, type PluginUiDef } from '@wisp/shell';
import { parseShareParams } from './share/params.ts';
import catalogue from './i18n/en.json' with { type: 'json' };

const ui: PluginUiDef = {
  id: 'wisp.stuff',
  catalogue,
  views: [
    { kind: 'stuff.page', slot: 'main', parse: () => null, component: lazy(() => import('./StuffPage.tsx')) as never, title: () => 'My Stuff' },
    // Share… a task (v1 share.js shareTask): from its row menu
    {
      kind: 'stuff.share',
      slot: 'overlay',
      overlay: 'dialog',
      parse: parseShareParams,
      component: lazy(() => import('./share/ShareDialog.tsx')) as never,
      title: () => (catalogue as Record<string, string>)['stuff.share.title'] ?? 'Share task',
      // the overlay is its dialog: v1's small share box, named as before
      size: 'sm',
      part: 'share.dialog',
    },
    // Make a pane (v1 pane-make.js): the head's Make a pane, ⌘K
    {
      kind: 'stuff.make',
      slot: 'overlay',
      overlay: 'dialog',
      parse: (p: unknown) => p,
      component: lazy(() => import('./MakePane.tsx')) as never,
      title: () => (catalogue as Record<string, string>)['stuff.make.title'] ?? 'Make a pane',
    },
  ],
  // apps pinned on Home (v1 split.js .sh-panes): after Spare allowance, before the tips
  // the tabs beside a task and Share… in its row menu: loaded as the first screen draws (ADR-0067)
  later: () => import('./later.ts'),
  extensions: [
    defineExtension({ id: 'wisp.stuff.homePanes', attachTo: { id: 'shell.home:cards' }, order: 50, component: lazy(() => import('./HomePanes.tsx')) }),
  ],
};
export default ui;
