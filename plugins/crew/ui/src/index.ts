// @wisp-plugin/crew-ui: wisp.crew's UI (docs/v2-ui-brief.md). Each extension's component loads with
// import() on first show, so the plugin costs nothing until it's on screen.

import { lazy } from 'solid-js';
import { defineExtension, defineView } from '@wisp/shell';
import { parseEditor } from './editor.ts';
import { t } from './i18n.ts';
import { VIEW } from './views.ts';

/** Share Ada, or Share your crew (the dialog's title; the member's name from its params). */
const shareTitle = (p: unknown): string => {
  const name = (p as { name?: unknown } | null)?.name;
  return typeof name === 'string' && name ? (t as (k: string, v?: Readonly<Record<string, string>>) => string)('wisp.crew.share.title', { name }) : t('wisp.crew.share.titleCrew');
};

import { definePluginUi } from './define.ts';
import { catalogue } from './i18n.ts';

export default definePluginUi({
  id: 'wisp.crew',
  // the row menu's item, the team tab beside a task, @member replies and ⌘K's actions: loaded as the first screen
  // draws (ADR-0067), with the crew's data model they read
  later: () => import('./later.ts'),
  // a member's page: the main pane when a face on the rail is picked (v1 crew.js buildView)
  extensions: [
    // the phone's Crew section at the top of the list (v1 #secCrew); it shows only in the stacked layout
    defineExtension({ id: 'wisp.crew.list', attachTo: { id: 'shell.list:crew' }, component: lazy(() => import('./CrewListTop.tsx')) }),
    // a face's card beside the rail (v1 .cl-card): its jobs, each a click away
    defineExtension({ id: 'wisp.crew.railCard', attachTo: { id: 'shell.rail:card' }, component: lazy(() => import('./RailCard.tsx')) }),
    defineExtension({ id: 'wisp.crew.page', attachTo: { id: 'shell.screen:crew' }, component: lazy(() => import('./CrewPage.tsx')) }),
    // under a turn: the members asked on it and their answers, with Reply (v1 btw.js crew entries)
    defineExtension({ id: 'wisp.crew.answers', attachTo: { id: 'shell.turn:foot' }, component: lazy(() => import('./Answers.tsx')) }),
    // Settings → Usage: Crew track record (v1 #openRecord)
    defineExtension({ id: 'wisp.crew.usage', attachTo: { id: 'shell.settings:usage' }, component: lazy(() => import('./UsageRow.tsx')) }),
  ],
  // add or edit a member (v1 crewEditor): the rail's +, a member page's Edit…
  views: [
    // Import crew members: a shared .crew.json, its JSON or a link (v1 crew-share.js importCrew)
    // Share…: a member or the whole crew as a .crew.json (v1 crew-share.js shareMember)
    defineView({ kind: VIEW.share, slot: 'overlay', overlay: 'dialog', parse: (p) => p, title: (p) => shareTitle(p), component: lazy(() => import('./ShareCrew.tsx')) }),
    defineView({ kind: VIEW.import, slot: 'overlay', overlay: 'dialog', parse: (p) => p, title: () => t('wisp.crew.import.title'), component: lazy(() => import('./ImportCrew.tsx')) }),
    // the crew's track record (v1 crew-record.js): Settings → Usage's Crew track record
    defineView({ kind: VIEW.record, slot: 'overlay', overlay: 'dialog', parse: (p) => p, title: () => t('wisp.crew.record.title'), component: lazy(() => import('./Record.tsx')) }),
    // Org: who hands work to whom (v1 org.js, F-crew-14), in the main pane
    defineView({ kind: VIEW.org, slot: 'main', parse: (p) => p, title: () => t('wisp.crew.org.title'), component: lazy(() => import('./OrgPage.tsx')) }),
    defineView({
      kind: VIEW.editor,
      slot: 'overlay',
      overlay: 'dialog',
      size: 'v1',
      parse: parseEditor,
      title: (p) => t(p.id ? 'wisp.crew.editor.edit' : p.task ? 'wisp.crew.editor.addTask' : 'wisp.crew.editor.add'),
      component: lazy(() => import('./CrewEditor.tsx')),
    }),
  ],
  commands: [],
  catalogue,
});
