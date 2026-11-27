// RunBoard: a multi-agent run as it goes (a workflow's steps, ADR-0119). Its phases are lanes left to right (one under
// the other when the box is narrow), each with how many of its steps ended; a step is a card (its state's mark, its
// title, the agent on it, how long it took and a line of what it gave or why not), one that runs shimmers, and a step
// that fans out over a list is one card with a dot per item, so 40 runs read at a glance. A card or a dot opens the
// step's task. Sheet: run-board.css, in the lazy chunk of what draws it (never the first screen).
import { For, Match, Show, Switch, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { Liquid } from '../content/Liquid.tsx';
import { Tooltip } from '../controls/Tooltip.tsx';
import { Menu } from '../controls/Menu.tsx';
import { ForKeyed } from '../core/keyed.tsx';
import type { IconName } from '../content/icons.ts';
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
  /** what you can do to it (Skip, Retry), in a ⋯ on the card */
  readonly actions?: readonly RunAction[];
  readonly onAction?: (value: string) => void;
}

/** One thing to do to a step: its menu entry. */
export interface RunAction {
  readonly value: string;
  readonly label: string;
  readonly icon?: IconName;
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
  /** what you can do to its runs (Retry the failed ones, Skip the rest) */
  readonly actions?: readonly RunAction[];
  readonly onAction?: (value: string) => void;
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
  /** the accessible name of a card's ⋯ */
  readonly actionsLabel?: string;
}

/** A card's ⋯: its actions, over the card's own open (never inside a button). */
function Actions(props: { readonly actions?: readonly RunAction[] | undefined; readonly onAction?: ((v: string) => void) | undefined; readonly label: string; readonly part: PartName }): JSX.Element {
  return (
    <Show when={(props.actions?.length ?? 0) > 0 && props.onAction}>
      <span data-kit="run-card-menu">
        <Menu
          part={props.part}
          trigger={{ icon: 'dots', 'aria-label': props.label, variant: 'ghost', size: 'xs' }}
          items={(props.actions ?? []).map((a) => ({ value: a.value, label: a.label, ...(a.icon ? { icon: a.icon } : {}) }))}
          onSelect={(v) => props.onAction?.(v)}
          placement="bottom-end"
        />
      </span>
    </Show>
  );
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
          <Liquid />
        </Match>
      </Switch>
    </span>
  );
}

function Card(props: { readonly step: RunStep; readonly part: PartName; readonly actionsLabel: string }): JSX.Element {
  return (
    <div data-kit="run-card" data-part={props.part} data-state={props.step.state} data-open={props.step.onOpen ? '' : undefined}>
      <Show when={props.step.onOpen}>
        {(open) => <button type="button" data-kit="run-card-open" aria-label={props.step.openLabel ?? props.step.title} onClick={() => open()()} />}
      </Show>
      <span data-kit="run-card-head">
        <Mark state={props.step.state} />
        <span data-kit="run-card-title">{props.step.title}</span>
        <Actions actions={props.step.actions} onAction={props.step.onAction} label={props.actionsLabel} part={sub(props.part, 'more')} />
      </span>
      <Show when={props.step.detail}>{(d) => <span data-kit="run-card-detail">{d()}</span>}</Show>
      <Show when={props.step.agent || props.step.meta}>
        <span data-kit="run-card-foot">
          <Show when={props.step.agent}>{(a) => <span data-kit="run-agent">{a()}</span>}</Show>
          <Show when={props.step.meta}>{(m) => <span data-kit="run-card-meta">{m()}</span>}</Show>
        </span>
      </Show>
    </div>
  );
}

function Fan(props: { readonly fan: RunFan; readonly part: PartName; readonly actionsLabel: string }): JSX.Element {
  return (
    <div data-kit="run-card" data-fan="" data-part={props.part} data-state={props.fan.state}>
      <span data-kit="run-card-head">
        <Mark state={props.fan.state} />
        <span data-kit="run-card-title">{props.fan.title}</span>
        <span data-kit="run-card-meta">{props.fan.count}</span>
        <Actions actions={props.fan.actions} onAction={props.fan.onAction} label={props.actionsLabel} part={sub(props.part, 'more')} />
      </span>
      <span data-kit="run-dots" role="list">
        <ForKeyed each={props.fan.items}>
          {(item) => (
            <span role="listitem" data-kit="run-dot-cell">
              <Tooltip content={item().title} {...(item().detail ? { hint: item().detail as string } : {})}>
                <Show when={item().onOpen} fallback={<span data-kit="run-dot" data-state={item().state} aria-label={item().title} />}>
                  {(open) => <button type="button" data-kit="run-dot" data-state={item().state} aria-label={item().openLabel ?? item().title} onClick={() => open()()} />}
                </Show>
              </Tooltip>
            </span>
          )}
        </ForKeyed>
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

export interface RunDotsProps extends KitProps {
  /** one per step run, in order; a title each for its tooltip */
  readonly items: readonly Pick<RunStep, 'id' | 'title' | 'state' | 'detail'>[];
  /** the strip's accessible name (`7 of 19 steps done`) */
  readonly label: string;
}

/** A run in one strip of dots, one per step run in order (a card beside the task, where a board doesn't fit). */
export function RunDots(props: RunDotsProps): JSX.Element {
  return (
    <span data-kit="run-dots" data-strip="" data-part={props.part ?? 'ui.runDots'} role="img" aria-label={props.label}>
      <For each={props.items}>{(item) => <span data-kit="run-dot" data-state={item.state} title={item.detail ? `${item.title}: ${item.detail}` : item.title} />}</For>
    </span>
  );
}

export interface RunChipProps extends KitProps {
  /** the run's name */
  readonly name: string;
  /** where it stands: running pulses its mark */
  readonly state: 'running' | 'done' | 'failed' | 'stopped';
  /** `4/9`, at the row's end */
  readonly progress: string;
  /** a segment per step run, in order: the bar under the row */
  readonly segments: readonly Pick<RunStep, 'id' | 'state'>[];
  /** its tooltip: the phase, how long, the cost */
  readonly title?: string;
  /** the accessible name of its open action */
  readonly label: string;
  readonly onOpen: () => void;
}

/** A run as a row of a task's details (its name and progress, a bar of a segment per step under it): a click opens its
 *  board. As compact as the details' other rows (the harness, the machine). */
export function RunChip(props: RunChipProps): JSX.Element {
  return (
    <button type="button" data-kit="run-chip" data-part={props.part ?? 'ui.runChip'} data-state={props.state} aria-label={props.label} title={props.title} onClick={() => props.onOpen()}>
      <span data-kit="run-chip-row">
        <span data-kit="run-chip-mark" aria-hidden="true">
          <Icon name="org" size="sm" />
        </span>
        <span data-kit="run-chip-name">{props.name}</span>
        <span data-kit="run-chip-progress">{props.progress}</span>
        <span data-kit="run-chip-go" aria-hidden="true">
          <Icon name="chevron" size="sm" />
        </span>
      </span>
      <span data-kit="run-chip-bar" aria-hidden="true">
        <For each={props.segments}>{(s) => <span data-kit="run-chip-seg" data-state={s.state} />}</For>
      </span>
    </button>
  );
}

export function RunBoard(props: RunBoardProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.runBoard';
  return (
    <div id={props.id} data-kit="run-board" data-part={part()} role="group" aria-label={props.label} data-lanes={props.lanes.length}>
      <ForKeyed each={props.lanes}>
        {(lane) => (
          <section data-kit="run-lane" data-part={sub(part(), 'lane')} data-state={lane().state} aria-label={lane().title || undefined}>
            <Show when={lane().title}>
              <header data-kit="run-lane-head">
                <span data-kit="run-lane-title">{lane().title}</span>
                <span data-kit="run-lane-count">{lane().count}</span>
              </header>
            </Show>
            <div data-kit="run-lane-cards">
              {/* by the step's id: the clock that moves each card's time never makes a card again (its entry, hover, menu) */}
              <ForKeyed each={lane().entries} by={(e) => ('fan' in e ? `fan:${e.fan.id}` : e.step.id)}>
                {(e) => {
                  const x = e();
                  return 'fan' in x ? (
                    <Fan fan={(e() as { fan: RunFan }).fan} part={sub(part(), 'fan')} actionsLabel={props.actionsLabel ?? 'Actions'} />
                  ) : (
                    <Card step={(e() as { step: RunStep }).step} part={sub(part(), 'step')} actionsLabel={props.actionsLabel ?? 'Actions'} />
                  );
                }}
              </ForKeyed>
            </div>
          </section>
        )}
      </ForKeyed>
    </div>
  );
}
