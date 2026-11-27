// Automations (design/machine-page.md §5); the last 24 hours (§6) are Overview.tsx's DayChart. Pause automations ▾ shows
// paused at once (the command's patch) and Resume now undoes it; the limits live in Settings →
// Usage → Machine load, one place, so Change goes there.

import type { PluginClient } from '@wisp/client';
import { Button, Menu, Section, SettingsRow } from '@wisp/ui';
import { Show, type JSX } from 'solid-js';
import { machineCommands } from './commands.ts';
import type { MachineData } from './data.ts';
import type { Host } from './host.ts';
import { automationsLine, batteryLine, limitsLine, pauseChoices } from './view.ts';
import './i18n/lazy.ts';

interface Props {
  readonly client: PluginClient;
  readonly host: Host;
  readonly data: MachineData;
  /** Settings → Usage → Machine load */
  readonly limits: () => void;
}

export function Automations(props: Props): JSX.Element {
  const t = props.client.t;
  const cmds = machineCommands(props.data.peer);
  const until = (): number => props.data.snap()?.guard.pausedUntil ?? 0;
  const paused = (): boolean => until() > props.data.now();
  const pause = (at: number): void => void props.client.run(cmds.pause, { until: at }).catch(() => {});
  const why = (): string => {
    const m = props.data.snap();
    if (!m) return '';
    if (paused()) return t('machine.auto.resumeHint');
    return batteryLine(m, t) || t('machine.auto.pauseHint');
  };
  const control = (): JSX.Element => (
    <Show
      when={!paused()}
      fallback={
        <Button variant="secondary" size="sm" onClick={() => pause(0)}>
          {t('machine.auto.resume')}
        </Button>
      }
    >
      <Menu
        trigger={{ label: t('machine.auto.pause'), variant: 'secondary', size: 'sm' }}
        items={pauseChoices(props.data.now(), t).map((c) => ({ value: String(c.at), label: c.label }))}
        onSelect={(v) => pause(Number(v))}
      />
    </Show>
  );
  return (
    <Section title={t('machine.auto.title')} description={t('machine.auto.desc')} part="machine.automations">
      <SettingsRow title={props.data.snap() ? automationsLine(props.data.snap(), 0, props.data.now(), t) : t('machine.at.reading')} description={why()} control={control()} />
      <SettingsRow
        title={t('machine.auto.limits')}
        description={props.data.snap() ? limitsLine(props.data.snap() as never, t) : ''}
        control={
          <Button variant="ghost" size="sm" onClick={props.limits}>
            {t('machine.act.change')}
          </Button>
        }
      />
    </Section>
  );
}
