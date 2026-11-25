// A harness's own stop line in its Settings → Agents detail (`shell.settings:harness`, harness audit r2): where
// unattended work stops on it (Same as Automations stop at, Off, 50, 75 or 90 %), with its week's usage line. It moved
// here from Settings → Usage's per-harness group, which links to it. Hidden for a harness whose CLI reports no quota
// window: its line would have nothing to stop at (ADR-0123), and the detail says so instead.
import { createResource, onCleanup, Show, type JSX } from 'solid-js';
import { entityKey } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { ChoiceSetting, SettingsGroup, SettingsNote, useSignal } from '@wisp/ui';
import { META, plans, setStop, TOPIC, usage, type AccountPlan } from './commands.ts';
import { HARNESSES, harnessesOf, harnessUsageDescription, limitsReported } from './plan.ts';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

const STOPS = ['same', 'off', '50', '75', '90'] as const;

export default function HarnessStop(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const peer = c.home;
  const scope = c.scope.child('harness-stop');
  onCleanup(() => scope.dispose());
  c.watch(peer, TOPIC, scope);
  const h = () => HARNESSES.find((x) => x.agent === props.id);
  const meta = useSignal(c.entities.row<{ body: Record<string, string> }>(META, entityKey(peer, 'settings')));
  const [windows] = createResource(
    () => props.id,
    (agent) => c.run(plans, { peer, agent }).catch(() => [] as AccountPlan[]),
  );
  const [line] = createResource(() => c.run(usage, { peer }).catch(() => undefined));
  const row = () => harnessesOf(line()).find((r) => (r.agent || r.key) === props.id);
  return (
    <Show when={h()}>
      {(meta0) => (
        <SettingsGroup part="budget.harness" title={t('budget.harness.title')}>
          {/* once the windows are in (so it doesn't flicker): a harness with none says so, no control */}
          <Show
            when={windows.latest === undefined || limitsReported(meta0(), windows.latest)}
            fallback={<SettingsNote part="budget.harness.none">{harnessUsageDescription(meta0(), row(), windows.latest ?? [])}</SettingsNote>}
          >
            <ChoiceSetting
              part="settings.autoStopBy"
              fieldset
              title={t('budget.harness.stop')}
              description={harnessUsageDescription(meta0(), row(), windows.latest ?? [])}
              source={{
                value: () => meta()?.body?.[meta0().key] ?? 'same',
                set: (v: string) => void c.run(setStop, { peer, key: meta0().key, value: v }).catch(() => {}),
              }}
              items={STOPS.map((v) => ({ value: v, label: t(`budget.usage.stop.${v}`) }))}
            />
          </Show>
        </SettingsGroup>
      )}
    </Show>
  );
}
