// RunBoard: a multi-agent run as it goes (a workflow's steps, ADR-0119). Its phases are lanes left to right (one under
// the other when the box is narrow), each with how many of its steps ended; a step is a card (its state's mark, its
// title, the agent on it, how long it took and a line of what it gave or why not), one that runs shimmers, and a step
// that fans out over a list is one card with a dot per item, so 40 runs read at a glance. A card or a dot opens the
// step's task. Sheet: run-board.css, in the lazy chunk of what draws it (never the first screen).
import { For, Match, Show, Switch, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { Tooltip } from '../controls/Tooltip.tsx';
import '../css/run-board.css';

export type RunStepState = 'waiting' | 'running' | 'done' | 'failed' | 'skipped';

export interface RunStep {
  readonly id: string;
  readonly title: string;
  readonly state: RunStepState;
  /** what it gave, or why it failed or was skipped (one line) */
  readonly detail?: string;
  /** the agent on it (`claude`, `codex · gpt-5`) */
  readonly agent?: string;
  /** how long it took, its cost (`1m 12s · $0.10`) */
  readonly meta?: string;
  /** the accessible name of its open action and its tooltip (`Open fix · a.rs`) */
  readonly openLabel?: string;
  readonly onOpen?: () => void;
}

/** A step that fans out over a list: one card, a dot per item. */
export interface RunFan {
  readonly id: string;
  readonly title: string;
  readonly state: RunStepState;
  readonly agent?: string;
  /** `8 of 12` */
  readonly count: string;
  readonly detail?: string;
  readonly items: readonly RunStep[];
}

export type RunEntry = { readonly step: RunStep } | { readonly fan: RunFan };

export interface RunLane {
  readonly id: string;
  /** the phase's name ('' for a run without phases: the lane has no header) */
  readonly title: string;
  /** `2/3` */
  readonly count: string;
  readonly state: RunStepState;
  readonly entries: readonly RunEntry[];
}

export interface RunBoardProps extends KitProps {
  readonly lanes: readonly RunLane[];
  /** the board's accessible name */
  readonly label: string;
}

function Mark(props: { readonly state: RunStepState }): JSX.Element {
  return (
    <span data-kit="run-mark" data-state={props.state} aria-hidden="true">
      <Switch>
        <Match when={props.state === 'done'}>
          <Icon name="check" size="sm" />
        </Match>
        <Match when={props.state === 'failed'}>
          <Icon name="x" size="sm" />
        </Match>
        <Match when={props.state === 'skipped'}>
          <Icon name="skip" size="sm" />
        </Match>
        <Match when={props.state === 'running'}>
          <span data-kit="run-live" />
        </Match>
      </Switch>
    </span>
  );
}

function Card(props: { readonly step: RunStep; readonly part: PartName }): JSX.Element {
  const body = (): JSX.Element => (
    <>
      <span data-kit="run-card-head">
        <Mark state={props.step.state} />
        <span data-kit="run-card-title">{props.step.title}</span>
      </span>
      <Show when={props.step.detail}>{(d) => <span data-kit="run-card-detail">{d()}</span>}</Show>
      <Show when={props.step.agent || props.step.meta}>
        <span data-kit="run-card-foot">
          <Show when={props.step.agent}>{(a) => <span data-kit="run-agent">{a()}</span>}</Show>
          <Show when={props.step.meta}>{(m) => <span data-kit="run-card-meta">{m()}</span>}</Show>
        </span>
      </Show>
    </>
  );
  return (
    <Show
      when={props.step.onOpen}
      fallback={
        <div data-kit="run-card" data-part={props.part} data-state={props.step.state}>
          {body()}
        </div>
      }
    >
      {(open) => (
        <button type="button" data-kit="run-card" data-part={props.part} data-state={props.step.state} aria-label={props.step.openLabel} onClick={() => open()()}>
          {body()}
        </button>
      )}
    </Show>
  );
}

function Fan(props: { readonly fan: RunFan; readonly part: PartName }): JSX.Element {
  return (
    <div data-kit="run-card" data-fan="" data-part={props.part} data-state={props.fan.state}>
      <span data-kit="run-card-head">
        <Mark state={props.fan.state} />
        <span data-kit="run-card-title">{props.fan.title}</span>
        <span data-kit="run-card-meta">{props.fan.count}</span>
      </span>
      <span data-kit="run-dots" role="list">
        <For each={props.fan.items}>
          {(item) => (
            <span role="listitem" data-kit="run-dot-cell">
              <Tooltip content={item.title} {...(item.detail ? { hint: item.detail } : {})}>
                <Show when={item.onOpen} fallback={<span data-kit="run-dot" data-state={item.state} aria-label={item.title} />}>
                  {(open) => <button type="button" data-kit="run-dot" data-state={item.state} aria-label={item.openLabel ?? item.title} onClick={() => open()()} />}
                </Show>
              </Tooltip>
            </span>
          )}
        </For>
      </span>
      <Show when={props.fan.detail}>{(d) => <span data-kit="run-card-detail">{d()}</span>}</Show>
      <Show when={props.fan.agent}>
        <span data-kit="run-card-foot">
          <span data-kit="run-agent">{props.fan.agent}</span>
        </span>
      </Show>
    </div>
  );
}

export function RunBoard(props: RunBoardProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.runBoard';
  return (
    <div id={props.id} data-kit="run-board" data-part={part()} role="group" aria-label={props.label} data-lanes={props.lanes.length}>
      <For each={props.lanes}>
        {(lane) => (
          <section data-kit="run-lane" data-part={sub(part(), 'lane')} data-state={lane.state} aria-label={lane.title || undefined}>
            <Show when={lane.title}>
              <header data-kit="run-lane-head">
                <span data-kit="run-lane-title">{lane.title}</span>
                <span data-kit="run-lane-count">{lane.count}</span>
              </header>
            </Show>
            <div data-kit="run-lane-cards">
              <For each={lane.entries}>
                {(e) => ('fan' in e ? <Fan fan={e.fan} part={sub(part(), 'fan')} /> : <Card step={e.step} part={sub(part(), 'step')} />)}
              </For>
            </div>
          </section>
        )}
      </For>
    </div>
  );
}
