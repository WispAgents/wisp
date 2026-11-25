// @wisp-plugin/sandbox-ui: wisp.sandbox's UI (ADR-0047). `/sandbox …` in New task opens a placeholder for the task at
// once (the Sandbox pill from the first frame) and a row on top of the task list while its sandbox comes up (a glowing
// grid; Claude's sign-in right there when it's missing), then the task itself; Sandbox in New task's
// Run on does the same with what's typed (TRY-19), offered while this wisp's grid makes sandboxes; Settings →
// Computers → Sandboxes: the Claude sign-in for every sandbox, env vars, and each sandbox with its owner.
import { createSignal, lazy } from 'solid-js';
import { uuidv7, type Json, type PeerId, type PluginClient } from '@wisp/client';
import { defineExtension, defineView, definePluginUi } from '@wisp/shell';
import { afterFirstScreen } from '@wisp/ui';
import { catalogue, t } from './i18n.ts';
import { sandboxPrompt } from './model.ts';
import { anyLaunch } from './launches.ts';
import { LAUNCHING, parseLaunching } from './views.ts';

// whether this wisp makes sandboxes (`sandbox.info`): asked when Run on is first drawn, and again after a minute, so
// signing in to a grid later shows it; the command's code loads with the question, not with the app
const [canMake, setCanMake] = createSignal(false);
/** when it was last asked: not a signal, so asking from inside a render doesn't run it again */
const asked = { at: -Infinity };
function offered(c: PluginClient): boolean {
  const now = Date.now();
  if (now - asked.at > 60_000) {
    asked.at = now;
    // once the first screen is up (ADR-0105): Sandbox joins Run on when the answer comes, as it always did
    afterFirstScreen(() =>
      void import('./commands.ts')
        .then(({ info }) => c.run(info, { peer: c.home as PeerId }))
        .then((i) => setCanMake(i.enabled), () => setCanMake(false)),
    );
  }
  return canMake();
}

/** `/sandbox …`: its launch row at once (the list and its page, optimistic), its page now, the sandbox behind them. */
const open = (c: PluginClient, prompt: string, launch: unknown): void => {
  const id = uuidv7();
  c.navigate({ t: 'openView', id: LAUNCHING as never, params: { id, prompt } as Json });
  void import('./commands.ts').then(({ start }) =>
    c.run(start, { peer: c.home as PeerId, id, at: Date.now(), args: { prompt, ...((launch ?? {}) as Record<string, unknown>) } }),
  );
};

export default definePluginUi({
  id: 'wisp.sandbox',
  catalogue,
  settings: [{ id: 'sandboxes', group: 'computers', title: 'sandbox.pane', icon: 'box', order: 25, component: lazy(() => import('./Sandboxes.tsx')) }],
  // the launches on top of the task list, only while there is one
  extensions: [defineExtension({ id: 'sandbox.launches', attachTo: { id: 'shell.list:top' }, if: (p) => anyLaunch(p.client), component: lazy(() => import('./LaunchRows.tsx')) })],
  views: [defineView({ kind: LAUNCHING, slot: 'main', parse: parseLaunching, title: () => t('sandbox.pill'), component: lazy(() => import('./Launching.tsx')) })],
  prompt: [
    {
      id: 'sandbox.slash',
      trigger: '/',
      where: ['home'],
      order: 95,
      items: () => [],
      send: (text, ctx) => {
        const prompt = sandboxPrompt(text);
        if (prompt === null || !prompt) return false;
        open(ctx.client, prompt, ctx.launch);
        return true;
      },
    },
  ],
  launchTargets: [{ id: 'sandbox.box', label: () => t('sandbox.pill'), icon: 'box', order: 10, offered, launch: (prompt, ctx) => open(ctx.client, prompt, ctx.launch) }],
});
