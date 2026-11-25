// A workflow's row in the open task's details column (ADR-0120), first among the plugins' rows (over the PR card), as
// compact as the harness and machine rows above it: the run's name and progress, a bar of a segment per step under it
// (it pulses while one works), its phase, time and cost in the tooltip. A click opens the run's board in the main area.
// On a step's own task the same row is its run (the parent's), so the way back is one click: task → board → step.

import { For, Show, createMemo, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { RunChip, Stack, useDerived } from '@wisp/ui';
import type { EntityKey, PluginClient } from '@wisp/client';
import { t } from './i18n.ts';
import { cardLine, dots, live, parentOf, runsOf, stepOf } from './model.ts';
import { keyOf, openBoard, useNow, useRuns } from './use.ts';
import { KIND, type RunView, type SessionRow } from './types.ts';
import './i18n/lazy.ts';

export default function WorkflowCard(props: ExtensionProps): JSX.Element {
  const k = (): EntityKey | undefined => {
    const id = props.id ?? props.session?.()?.id;
    return id ? keyOf(props.client, id) : undefined;
  };
  return (
    <Show when={k()} keyed>
      {(key) => <Rows client={props.client} k={key} />}
    </Show>
  );
}

/** The runs to show as rows: the task's own (the ones going, else the newest), or the run a step of it belongs to. */
function Rows(props: { readonly client: PluginClient; readonly k: EntityKey }): JSX.Element {
  const { row, runs } = useRuns(props.client, props.k);
  const step = createMemo(() => stepOf(row()));
  // a step's task: its run is on its parent (the run's task)
  const owner = createMemo((): EntityKey | undefined => {
    const p = parentOf(row());
    if (!step() || !p) return undefined;
    return keyOf(props.client, p.includes(':') ? p : `${props.k.slice(0, props.k.indexOf(':'))}:${p}`);
  });
  const ownerRuns = useDerived(() => {
    const o = owner();
    return o ? runsOf(props.client.entities.row<SessionRow>(KIND.session, o)() as SessionRow | undefined) : [];
  });
  const shown = createMemo((): { task: EntityKey; run: RunView; more: number }[] => {
    const mine = runs();
    if (mine.length > 0) {
      const going = mine.filter(live);
      const list = going.length > 0 ? going : mine.slice(-1);
      return list.map((run) => ({ task: props.k, run, more: mine.length - list.length }));
    }
    const s = step();
    const o = owner();
    const run = ownerRuns().find((r) => r.id === s?.run);
    return o && run ? [{ task: o, run, more: 0 }] : [];
  });
  const now = useNow(props.client, () => shown().some((x) => live(x.run)));
  return (
    <Show when={shown().length > 0}>
      <Stack gap="1" part="workflows.rows">
        <For each={shown()}>
          {(x) => (
            <RunChip
              part="workflows.row"
              name={step() && x.task !== props.k ? t('wisp.workflows.stepOf', { node: step()?.node ?? '', name: x.run.workflow }) : x.run.workflow}
              state={x.run.state}
              progress={`${x.run.done ?? 0}/${x.run.total ?? x.run.steps.length}`}
              segments={dots(x.run)}
              title={cardLine(x.run, now(), x.more)}
              label={t('wisp.workflows.cardLabel', { name: x.run.workflow })}
              onOpen={() => openBoard(props.client, x.task, x.run.id)}
            />
          )}
        </For>
      </Stack>
    </Show>
  );
}
