// wisp.crew's UI, the part only a press reaches (ADR-0067: PluginUiDef.later): loaded as the first screen draws.
import { lazy } from 'solid-js';
import type { PluginUiLater } from '@wisp/shell';
import { defineExtension } from './define.ts';
import { askedOf, askAll } from './answers.ts';
import { askCrew } from './commands.ts';
import { t } from './i18n.ts';
import { VIEW } from './views.ts';
import { crewData } from './data.ts';
import { hasTeam } from './team.ts';
import { hasSubagents, subagentData } from './subagents.ts';
import './i18n/lazy.ts';

interface RowLike {
  readonly id: string;
  readonly agent?: string;
  readonly parentId?: string;
  readonly pending?: boolean;
  readonly crew?: { readonly id: string };
}
const later: PluginUiLater = {
  // the lead's team beside a task that handed crew jobs out (v1 team.js, F-crew-11): a tab only while it has some
  sideTabs: [
    {
      id: 'wisp.crew.team',
      // and beside one with subagents of its own: wisp's delegates, the harness's own (TRY-24)
      tabs: ({ client, task }) =>
        hasTeam(crewData(client).jobs(), task) ||
        hasSubagents(subagentData(client).tasks(), subagentData(client).steps(), task)
          ? [{ value: 'team', label: t('wisp.crew.team.tab'), title: t('wisp.crew.team.tip') }]
          : [],
      component: lazy(() => import('./TeamPane.tsx')),
    },
  ],
  extensions: [
    // a task's row menu (v1 actions.js crewItem): a member's job opens them; a task of yours can become a member's
    defineExtension({
      id: 'wisp.crew.rowCrew',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 100,
      if: (p) => {
        const s = p.session?.() as RowLike | undefined;
        return !!s && (!!s.crew?.id || !(s.parentId || s.agent === 'wisp' || s.pending));
      },
      bar: {
        icon: 'crew',
        label: 'wisp.crew.row.add',
        priority: 100,
        section: 'look',
        text: (p) => {
          const s = p.session?.() as RowLike | undefined;
          const m = s?.crew?.id ? crewData(p.client).byId(s.crew.id) : undefined;
          return m ? (t as (k: string, v?: Readonly<Record<string, string>>) => string)('wisp.crew.row.open', { name: m.name }) : t('wisp.crew.row.add');
        },
        select: (p) => {
          const s = p.session?.() as RowLike | undefined;
          if (!s) return;
          if (s.crew?.id) return p.client.navigate({ t: 'openCrew', id: s.crew.id as never });
          p.client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: VIEW.editor as never, params: { task: s.id } } });
        },
      },
    }),
  ],
  // a reply that starts with @a member is for them: their job, their answer under the turn (v1 crew.js replyIn)
  prompt: [
    {
      id: 'wisp.crew.ask',
      trigger: '@',
      where: ['reply'],
      order: 90,
      items: () => [],
      send: (text, ctx) => {
        const asked = askedOf(text);
        if (!asked) return false;
        askAll(asked, (id, question) => void ctx.client.run(askCrew, { task: ctx.task as never, id, question }).catch(() => undefined));
        return true;
      },
    },
  ],
  // ⌘K's Actions: share the whole crew as a link or a file (ADR-0052), and take one in (a link, v1's gist or a file)
  palette: [
    {
      id: 'wisp.crew.palette',
      order: 61,
      items: (ctx) => [
        {
          id: 'wisp.crew.share',
          group: 'shell.palette.group.actions',
          label: t('wisp.crew.palette.share'),
          icon: 'share',
          keywords: 'crew members share link export gist file team',
          run: () => ctx.client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: VIEW.share as never, params: {} } }),
        },
        {
          id: 'wisp.crew.import',
          group: 'shell.palette.group.actions',
          label: t('wisp.crew.palette.import'),
          icon: 'download',
          keywords: 'crew members import add link gist file crew.yml yaml crew.json',
          run: () => ctx.client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: VIEW.import as never, params: {} } }),
        },
      ],
    },
  ],
};
export default later;
