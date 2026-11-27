// The open task's details column as v1 draws it (panel.css .it-bar, .it-actions): where it runs, its branch and the
// task's actions.
import { Index, Show, createSignal, on, type JSX } from 'solid-js';
import { ForKeyed } from '../core/keyed.tsx';
import { flag } from '../core/props.ts';
import { ActIcon, WithAct, type Act } from './act.tsx';
import { createTimer } from '../blocks/content-scope.ts';
import { copyText } from '../blocks/content-copy.tsx';
import { Tooltip } from '../controls/Tooltip.tsx';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { TaskUsage, type TaskUsageProps } from './TaskUsage.tsx';

export interface TaskAction {
  readonly id: string;
  readonly icon: IconName;
  readonly label: string;
  readonly title: string;
  readonly part?: string;
  readonly tone?: 'good';
  /** its key, shown at the row's end */
  readonly key?: string;
  /** desktop only: on a phone it sits in More */
  readonly deskOnly?: boolean;
  readonly act?: Act;
  /** what it shows is open (v1 .ghost.tx-toggle.on: the Transcript action while the transcript shows). Additive. */
  readonly on?: boolean;
}

/** One folder the task works in, on its page (TaskDetails `places`). */
export interface TaskPlaceChip {
  readonly name: string;
  readonly color: string;
  /** its path, and whether the task starts there or also works in it */
  readonly title?: string;
  /** a folder it also works in (not the one it started in) */
  readonly also?: boolean;
  /** opens that project; none for a folder in no project */
  readonly onOpen?: () => void;
}

export interface TaskDetailsProps {
  readonly harness: string;
  readonly logo?: string;
  readonly project: string;
  readonly color: string;
  /**
   * every folder the task works in (v1 places.js), its own first: one chip each, a click opens that project. When set it
   * replaces `project`/`color`. Additive (F-task-27).
   */
  readonly places?: readonly TaskPlaceChip[];
  /** the state, shown on a phone (the list says it beside the task on a desktop) */
  readonly status?: { readonly label: string; readonly tone: string };
  /** a plan's when (v1 plan-view .pv-when): Starts in 2h, At 3pm, Draft, After “x”; first in the column */
  readonly when?: { readonly icon: IconName; readonly label: string; readonly title?: string };
  readonly branch?: {
    readonly head: string;
    readonly tail: string;
    readonly title: string;
    /** a click copies `text` (the worktree's path) and says `done` over the chip for a moment */
    readonly copy?: { readonly text: string; readonly done: string };
  };
  /** the machine it runs on (this computer, a peer, a sandbox), over the branch. Additive. */
  readonly machine?: { readonly name: string; readonly icon: IconName; readonly title?: string };
  /**
   * the wide column's words before the project and the machine ("in" wisp, "on" This computer), so the column reads as
   * one sentence: harness, in project, on computer, branch, then usage. The narrow bar and the phone sheet don't show
   * them. Additive.
   */
  readonly leads?: { readonly project: string; readonly machine: string };
  /** its usage, its subagents' in, and how full its context is (v1 #785): the wide pane's lines. Additive. */
  readonly usage?: TaskUsageProps;
  readonly actions: readonly TaskAction[];
  /**
   * what plugins pin to the column (`shell.task:details`: wisp.ship's PR card), last in it so it stays in sight while the
   * turns scroll; a full row under the narrow pane's bar. Additive.
   */
  readonly extra?: JSX.Element;
}

const COPIED_MS = 1200;

/** The task's branch; with `copy`, a button that copies the worktree's path and says so in a tooltip. */
function BranchChip(props: NonNullable<TaskDetailsProps['branch']>): JSX.Element {
  const timer = createTimer();
  const [copied, setCopied] = createSignal(false);
  const chip = (): JSX.Element => (
    <>
      <Icon name="branch" />
      <span data-kit="b-h">{props.head}</span>
      <span data-kit="b-t">{props.tail}</span>
    </>
  );
  return (
    <Show when={props.copy} fallback={<span data-kit="m-branch" title={props.title}>{chip()}</span>}>
      {(c) => (
        <Tooltip content={c().done} open={copied()} disabled={!copied()} placement="bottom" part="split.branch-copied">
          <button
            type="button"
            data-kit="m-branch"
            data-part="split.branch"
            title={copied() ? undefined : props.title}
            onClick={() => {
              void copyText(c().text).then((ok) => {
                if (!ok) return;
                setCopied(true);
                timer.after(COPIED_MS, () => setCopied(false));
              });
            }}
          >
            {chip()}
          </button>
        </Tooltip>
      )}
    </Show>
  );
}

/** v1 .it-bar: the harness, in its project, on its computer, the branch, the usage, then the task's actions. */
export function TaskDetails(props: TaskDetailsProps): JSX.Element {
  return (
    <div data-kit="it-bar" data-part="split.details">
      <div data-kit="it-props">
        <Show when={props.when}>
          {(w) => (
            <span data-kit="it-when" data-part="split.when" title={w().title}>
              <Icon name={w().icon} />
              <span>{w().label}</span>
            </span>
          )}
        </Show>
        <Show when={props.status}>
          {(st) => (
            <span data-kit="it-state" data-tone={st().tone}>
              <i />
              {st().label}
            </span>
          )}
        </Show>
        <span data-kit="model-logo" role="img" aria-label={props.harness} title={props.harness}>
          <Show when={props.logo}>{(src) => <img src={src()} alt="" />}</Show>
          <span data-kit="ml-name">{props.harness}</span>
        </span>
        {/* in its project: one line in the wide column ("📁 in wisp", the folder in the project's colour), the chips as they
            were in the narrow bar */}
        <span data-kit="it-in" data-part="split.details.project" data-color={props.places?.[0]?.color ?? props.color}>
          <Show when={props.leads}>
            <Icon name="folder" />
          </Show>
          <Show when={props.leads}>{(l) => <span data-kit="it-lead">{l().project}</span>}</Show>
          <Show
            when={props.places}
            fallback={
              <span data-kit="q-proj" data-color={props.color}>
                {props.project}
              </span>
            }
          >
            {(places) => (
              // by place, each chip made once (the row changes while the task runs: B31)
              <Index each={places()}>
                {(p) => (
                  <Show
                    when={p().onOpen}
                    fallback={
                      <span data-kit="q-proj" data-part="split.place" data-color={p().color} data-also={flag(p().also)} title={p().title}>
                        {p().name}
                      </span>
                    }
                  >
                    {(open) => (
                      <button type="button" data-kit="q-proj" data-part="split.place" data-color={p().color} data-also={flag(p().also)} title={p().title} onClick={() => open()()}>
                        {p().name}
                      </button>
                    )}
                  </Show>
                )}
              </Index>
            )}
          </Show>
        </span>
      </div>
      <Show when={props.branch || props.machine}>
        <div data-kit="it-meta">
          <Show when={props.machine}>
            {(m) => (
              <span data-kit="m-where" data-part="split.machine" title={m().title ?? m().name}>
                <Icon name={m().icon} />
                <Show when={props.leads}>{(l) => <span data-kit="it-lead">{l().machine}</span>}</Show>
                <span>{m().name}</span>
              </span>
            )}
          </Show>
          <Show when={props.branch}>
            {(b) => <BranchChip {...b()} />}
          </Show>
        </div>
      </Show>
      {/* the usage after where it runs (owner 2026-11-28: harness, in project, on computer, branch, then tokens and
          context); it shows in the wide column only (app-usage.css) */}
      <Show when={props.usage}>
        {(u) => (
          <div data-kit="it-usage" data-part="split.details.usage">
            <TaskUsage {...u()} />
          </div>
        )}
      </Show>
      <Show when={props.extra}>
        <div data-kit="it-extra" data-part="split.extra">
          {props.extra}
        </div>
      </Show>
      {/* the actions moved to the head (TaskHead `bar`); a host that still hands them gets them here */}
      <Show when={props.actions.length > 0}>
      <div data-kit="it-actions" data-part="split.actions">
        {/* by id, each button made once: the task's row changes a few times a second while it runs, and each change
            hands a new list (B31: every button was taken down and put back, its hover and focus lost) */}
        <ForKeyed each={props.actions}>
          {(a) => {
            return (
              <WithAct
                act={a().act}
                render={(trigger) => (
                  <button
                    type="button"
                    {...trigger()}
                    data-kit="ghost-xs"
                    data-a={a().id}
                    data-part={a().part}
                    data-tone={a().tone}
                    data-key={a().key}
                    data-desk={flag(a().deskOnly)}
                    data-on={flag(a().on)}
                    aria-pressed={a().on === undefined ? undefined : a().on}
                    title={a().title}
                  >
                    <ActIcon act={a().act}>
                      <Icon name={a().icon} />
                    </ActIcon>
                    <span data-text="control">{a().label}</span>
                  </button>
                )}
              />
            );
          }}
        </ForKeyed>
      </div>
      </Show>
    </div>
  );
}
