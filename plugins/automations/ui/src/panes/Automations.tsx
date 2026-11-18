// Settings → Automations (v1 settings.js automations, design/v1-reference/settings.automations): one group, as
// v1 had it: Recipes and Triggers, each a line saying what there is with Manage, which opens the Automations view
// (AutomationsPage: v1's lists with their actions and its trigger form), and Webhook triggers (other services may
// start this computer's webhook triggers; off, those URLs refuse every call).
import { createResource, createSignal, Match, Switch, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { Button, SettingsGroup, SettingsRow, SwitchSetting, type SettingSource } from '@wisp/ui';
import { recipes, triggers, type Recipe, type Trigger } from '../commands.ts';
import AutomationsPage from '../AutomationsPage.tsx';
import { t } from '../i18n.ts';
import { fromRow } from '../source.ts';
import { summary } from './summary.ts';
import '../i18n/lazy.ts';

/** `automations.webhookTriggers`, a bool setting (off by default): a boolean source, 'off' read as off. */
function webhooks(c: PluginClient): SettingSource<boolean> {
  const s = fromRow(c, 'automations.webhookTriggers', false);
  return {
    value: () => {
      const v = (s.value as () => unknown)();
      return v === true || v === 'on';
    },
    ...(s.lockedBy && { lockedBy: s.lockedBy }),
    set: (on) => s.set(on),
  };
}

/** `filter` (Settings' param): the tab it opens on, as v1's ⌘K Recipes and Triggers opened Automations on theirs */
export default function Automations(props: { readonly client: PluginClient; readonly filter?: string }): JSX.Element {
  const c = props.client;
  const peer = c.home;
  const [rs] = createResource(() => c.run(recipes, { peer }).catch((): Recipe[] => []));
  // the user's triggers: a crew member's wakes (`from` wisp.crew) are the crew page's
  const [ts] = createResource(() => c.run(triggers, { peer }).then((l) => l.filter((x) => x.from === 'trigger')).catch((): Trigger[] => []));
  const [view, setView] = createSignal<'home' | 'recipes' | 'triggers'>(props.filter === 'recipes' || props.filter === 'triggers' ? props.filter : 'home');
  const back = (): JSX.Element => (
    <Button size="sm" variant="ghost" icon="back" part="automations.back" onClick={() => setView('home')}>
      {t('automations.back')}
    </Button>
  );
  return (
    <Switch>
      <Match when={view() === 'home'}>
        <SettingsGroup part="automations.home">
          <SettingsRow
            part="automations.recipes.line"
            title={t('automations.recipes')}
            description={summary(
              (rs() ?? []).map((r) => r.name),
              t('automations.recipes.none').toLowerCase(),
            )}
            control={
              <Button size="sm" variant="ghost" onClick={() => setView('recipes')}>
                {t('automations.manage')}
              </Button>
            }
          />
          <SettingsRow
            part="automations.triggers.line"
            title={t('automations.triggers')}
            description={summary(
              (ts() ?? []).map((x) => x.name),
              t('automations.triggers.short'),
            )}
            control={
              <Button size="sm" variant="ghost" onClick={() => setView('triggers')}>
                {t('automations.manage')}
              </Button>
            }
          />
          <SwitchSetting
            part="settings.webhookTriggers"
            title={t('automations.webhooks')}
            description={t('automations.webhooks.description')}
            source={webhooks(c)}
            locked={(by) => t('automations.locked', { by })}
          />
        </SettingsGroup>
      </Match>
      {/* Manage (v1 settings.js data-auto): the Automations view itself, as v1 opened openAutomations in the pane, so
          its recipes and triggers are run, scheduled, edited, switched off and deleted here too (audit A #8, #9) */}
      <Match when={view() !== 'home'}>
        {back()}
        <AutomationsPage client={c} params={{ tab: view() === 'triggers' ? 'triggers' : 'recipes' }} />
      </Match>
    </Switch>
  );
}
