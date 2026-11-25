// wisp.workflows' UI, the part a prompt box reaches (ADR-0067: PluginUiDef.later): `/workflow` in both boxes.
import type { PluginUiLater } from '@wisp/shell';
import { savedOn, suggestions } from './suggest.ts';

const later: PluginUiLater = {
  prompt: [
    {
      id: 'wisp.workflows.slash',
      trigger: '/',
      order: 20,
      items: (query, ctx) => {
        const home = ctx.client.home as string;
        const peer = ctx.task ? ctx.task.slice(0, ctx.task.indexOf(':')) || home : home;
        return suggestions(query, query.includes(' ') ? savedOn(ctx.client, peer) : []);
      },
    },
  ],
};
export default later;
