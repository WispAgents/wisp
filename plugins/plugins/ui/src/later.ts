// wisp.plugins' UI, the part only a press reaches (ADR-0067: PluginUiDef.later): loaded as the first screen draws.
// `/plugin <ask>` casts a plugin as Settings → Plugins' box does: a task whose agent writes it as a plugin of yours
// and installs it off, opened at once. `/plugin` alone opens Settings → Plugins, where the examples are.
import type { PeerId, PluginClient } from '@wisp/client';
import type { LaunchPick, PluginUiLater } from '@wisp/shell';
import { cast, castPrompt } from './commands.ts';
import { t } from './i18n.ts';
import { pluginAsk, pluginItems } from './slash.ts';

function send(text: string, ctx: { client: PluginClient; launch?: LaunchPick; restore?: (text: string) => void; say?: (text: string) => void }): boolean {
  const ask = pluginAsk(text);
  if (ask === null) return false;
  const c = ctx.client;
  if (!ask) {
    c.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'plugins' } } });
    return true;
  }
  // New task's agent pick goes with it (Auto is the router's, never sent as an agent)
  const l = ctx.launch ?? {};
  const pick = l.agent && l.agent !== 'auto' ? { agent: l.agent, ...(l.model && { model: l.model }), ...(l.effort && { effort: l.effort }) } : {};
  // the box clears now; New task says what started (the reply box opens the new task when it's made)
  ctx.say?.(t('plugins.cast.started'));
  c.run(cast, { peer: c.home as PeerId, prompt: castPrompt(ask), title: ask, ext: { 'wisp.plugins': { kind: 'cast' } }, ...pick }).then(
    (r) => c.navigate({ t: 'openTask', id: r.id as never }),
    // refused: the toast says why, and what was typed is back in the reply box
    () => ctx.restore?.(text),
  );
  return true;
}

const later: PluginUiLater = {
  prompt: [
    {
      id: 'wisp.plugins.slash',
      trigger: '/',
      where: ['home', 'reply'],
      order: 50,
      items: (q) => pluginItems(q, (k) => t(k as 'plugins.slash.hint')),
      send,
    },
  ],
};
export default later;
