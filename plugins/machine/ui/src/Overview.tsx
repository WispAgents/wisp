// The Overview tab (ADR-0144, the owner's pick B): one line on top that says whether this computer is fine and why,
// then memory, CPU and disk as bars with the mark where automations start to wait, and the last 24 hours as two lines
// and one sentence. What needs you (left behind) and what runs now sit between them, in MachinePane.

import type { PluginClient } from '@wisp/client';
import { Callout, DayLines, LoadRow, Section, Skeleton, Stack } from '@wisp/ui';
import { Index, Show, createMemo, type JSX } from 'solid-js';
import type { MachineData } from './data.ts';
import { clock, dayLine, dayLines, loads, statusOf } from './view.ts';
import './i18n/lazy.ts';

interface Props {
  readonly client: PluginClient;
  readonly data: MachineData;
}

export function StatusLine(props: Props): JSX.Element {
  const t = props.client.t;
  const status = createMemo(() => {
    const m = props.data.snap();
    return m ? statusOf(m, props.data.now(), t) : undefined;
  });
  return (
    <Show when={status()} fallback={<Skeleton shape="block" width="full" height="touch" />}>
      {(s) => (
        <Callout tone={s().tone} title={s().title} part="machine.status">
          {s().detail}
        </Callout>
      )}
    </Show>
  );
}

export function Loads(props: Props): JSX.Element {
  const t = props.client.t;
  const rows = createMemo(() => loads(props.data.snap(), t));
  return (
    <Stack gap="0" part="machine.loads">
      {/* by position: a sample updates the three in place */}
      <Index each={rows()}>
        {(r) => (
          <LoadRow
            name={r().name}
            share={r().share}
            mark={r().mark}
            value={r().value}
            sub={r().sub}
            tone={r().tone}
            {...(r().mark !== null && { markLabel: r().sub })}
            part={`machine.load.${r().key}`}
          />
        )}
      </Index>
    </Stack>
  );
}

export function DayChart(props: Props): JSX.Element {
  const t = props.client.t;
  const lines = createMemo(() => {
    const m = props.data.snap();
    if (!m) return undefined;
    const d = dayLines(m, props.data.now());
    return {
      caption: dayLine(m, props.data.now(), t),
      axis: [clock(d.from, props.data.now()), t('machine.day.now')] as const,
      lines: [
        { key: 'memory', label: t('machine.day.memoryUsed'), points: d.memory, tone: 'accent' as const },
        { key: 'cpu', label: t('machine.gauge.cpu'), points: d.cpu, tone: 'warn' as const },
      ],
    };
  });
  return (
    <Section title={t('machine.day.title')} part="machine.day">
      <Show when={lines()} fallback={<Skeleton shape="block" width="full" height="touch" />}>
        {(l) => <DayLines label={t('machine.day.title')} lines={l().lines} caption={l().caption} axis={l().axis} part="machine.day.lines" />}
      </Show>
    </Section>
  );
}
