// The reply box's parts (ReplyBox.tsx): its goal line, its extras and each group of its tools row. Each reads the
// box's own props, as the box does.
import { For, Show, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { ActIcon, WithAct } from './act.tsx';
import { ReplyOptions, type ReplyOptionLabels } from './ReplyOptions.tsx';
import { Icon } from '../content/Icon.tsx';
import { Button } from '../controls/Button.tsx';
import type { ReplyBoxProps } from './reply-box-props.ts';

/** The full box's props, as ReplyBox reads them (a compact box draws none of the tools). */
export type BoxProps = ReplyBoxProps & { readonly compact?: boolean; readonly part?: string };

/** Done when: the goal line under the text (v1 .l-goal), its Enter and Escape, and Set. */
export function ReplyGoalLine(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show when={props.goalLine}>
      {(g) => (
        <div data-kit="l-goal" data-in="reply">
          <span data-kit="lg-k">
            <Icon name="target" />
            <span data-text="control">{g().label}</span>
          </span>
          <input
            data-kit="l-goal-input"
            ref={(el) => g().ref?.(el)}
            aria-label={g().label}
            placeholder={g().placeholder}
            value={g().value}
            onInput={(e) => g().onInput(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape' && g().onEscape) {
                e.preventDefault();
                e.stopPropagation();
                g().onEscape?.();
              } else if (e.key === 'Enter' && !e.isComposing && g().onEnter) {
                e.preventDefault();
                g().onEnter?.();
              }
            }}
          />
          <Show when={g().keys}>
            <span data-kit="lg-keys">{g().keys}</span>
          </Show>
          <Show when={g().set}>
            {(s) => (
              <Button variant="accent" size="xs" part="composer.goalSet" title={s().title} onClick={() => s().onClick()}>
                {s().label}
              </Button>
            )}
          </Show>
        </div>
      )}
    </Show>
  );
}

/** What goes with the reply (its files, a phone's Send later time), each with its ✕. */
export function ReplyExtras(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show when={(props.extras?.length ?? 0) > 0}>
      <div data-kit="l-extras">
        <For each={props.extras}>
          {(x) => (
            <span data-kit="lp-chip" data-extra="">
              <Icon name={x.icon} />
              <span data-text="control">{x.label}</span>
              <button type="button" data-kit="chip-x" aria-label={x.removeLabel} title={x.removeLabel} onClick={() => x.onRemove()}>
                <Icon name="x" />
              </button>
            </span>
          )}
        </For>
      </div>
    </Show>
  );
}

/** `+` for what's off (the desktop's one footer, UX 2026-11-18 #3), else the bare attach button. */
export function ReplyAttach(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show
      when={props.plus}
      fallback={
        <WithAct
          act={props.attachAct}
          render={(trigger) => (
            <button type="button" {...trigger()} data-kit="icon-btn" data-part="composer.attach" title={props.attach} aria-label={props.attach}>
              <ActIcon act={props.attachAct}>
                <Icon name="clip" />
              </ActIcon>
            </button>
          )}
        />
      }
    >
      {(p) => (
        <WithAct
          act={p().act}
          render={(trigger) => (
            <button type="button" {...trigger()} data-kit="icon-btn" data-a="plus" data-part="composer.plus" title={p().title} aria-label={p().label}>
              <Icon name="plus" />
            </button>
          )}
        />
      )}
    </Show>
  );
}

/** The phone app's mic (v1 dictate.js). */
export function ReplyMic(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show when={props.mic}>
      {(m) => (
        <WithAct
          act={m().act}
          render={(trigger) => (
            <button type="button" {...trigger()} data-kit="icon-btn" data-part="composer.mic" data-on={flag(m().on)} aria-pressed={m().on} title={m().label} aria-label={m().label}>
              <Icon name="mic" />
            </button>
          )}
        />
      )}
    </Show>
  );
}

/** The mode, and a channel's cap after it (F-chan-10) with Lift. */
export function ReplyMode(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <>
      <WithAct
        act={props.mode.act}
        render={(trigger) => (
          <button type="button" {...trigger()} data-kit="mode-btn" data-part="composer.mode" data-reply="" title={props.mode.title} aria-label={props.mode.label}>
            <Icon name={props.mode.icon ?? 'spark'} />
            <span data-text="control">{props.mode.label}</span>
          </button>
        )}
      />
      <Show when={props.mode.cap}>
        {(c) => (
          <span data-kit="c-cap" data-part="composer.cap" title={c().title}>
            <Icon name="lock" size="xs" />
            <span data-kit="c-cap-words" data-text="control">
              {c().label}
            </span>
            <WithAct
              act={c().lift.act}
              render={(trigger) => (
                <button type="button" {...trigger()} data-kit="c-cap-lift" data-part="composer.capLift" title={c().lift.title}>
                  <span data-text="control">{c().lift.label}</span>
                </button>
              )}
            />
          </span>
        )}
      </Show>
    </>
  );
}

/** The harness · model the reply runs on, with its plan ring past 80% (UX 2026-11-18 #8). */
export function ReplyRunner(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show when={props.runner}>
      {(r) => (
    <WithAct
      act={r().act}
      render={(trigger) => (
        <button
          type="button"
          {...trigger()}
          data-kit="c-runner"
          data-part="composer.harness"
          aria-haspopup="dialog"
          aria-expanded={r().open ? 'true' : 'false'}
          title={r().usage ? `${r().title}\n${r().usage?.title ?? ""}` : r().title}
          aria-label={r().usage ? `${r().title}\n${r().usage?.title ?? ""}` : r().title}
        >
          <span data-kit="rb-logo" data-bare="">
            <Show when={r().logo}>{(src) => <img src={src()} alt="" />}</Show>
            <Show when={r().usage}>{(u) => <span data-kit="rb-ring" data-part="composer.harness.usage" data-tone={u().tone} aria-hidden="true" style={{ '--ctx': `${u().pct}%` }} />}</Show>
          </span>
          <Show when={r().model}>
            {(m) => (
              <span data-kit="c-model" data-text="control">
                {m()}
              </span>
            )}
          </Show>
        </button>
      )}
    />
      )}
    </Show>
  );
}

/** Delegate and Goal as buttons, where there's no `+` to hold them. */
export function ReplyTeamGoal(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show when={!props.plus}>
    <WithAct
      act={props.teamAct}
      render={(trigger) => (
        <button type="button" {...trigger()} data-kit="c-team" data-part="composer.team" title={props.team} aria-label={props.team}>
          <Icon name="agent" />
        </button>
      )}
    />
    <WithAct
      act={props.goalAct}
      render={(trigger) => (
        <button
          type="button"
          {...trigger()}
          data-kit="icon-btn"
          data-a="goal"
          data-part="composer.goal"
          data-on={flag(props.goalOn)}
          aria-pressed={props.goalOn === true}
          title={props.goal}
          aria-label={props.goal}
        >
          <Icon name="target" />
        </button>
      )}
    />
    </Show>
  );
}

/** The hint beside the tools (the cold cache, a factory run): its words, or a link alone. */
export function ReplyHint(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <Show when={props.hint} fallback={<span data-kit="grow" />}>
      {(h) => (
        // no words, only a link (a phone's factory run: Stop the run): the link alone, whole, never cut by the
        // words' container (audit G tasks #6)
        <span data-kit="c-hint" data-part="composer.hint" data-bare={flag(!h().long && !!h().link)}>
          <span data-kit="c-cold-row">
            <Show when={h().long}>
              <span data-kit="c-cold" title={h().title ?? h().long}>
                <span data-kit="cc-long">{h().long}</span>
                <span data-kit="cc-short">{h().short}</span>
              </span>
            </Show>
            <Show when={h().link}>
              {(l) => (
                <WithAct
                  act={l().act}
                  render={(trigger) => (
                    <button type="button" {...trigger()} data-kit="link" data-part="composer.hint.link" title={l().title ?? l().label}>
                      {l().label}
                    </button>
                  )}
                />
              )}
            </Show>
          </span>
        </span>
      )}
    </Show>
  );
}

/** Queue (a follow-up waits behind the run) and Send later, where there's no `+`. */
export function ReplyQueueLater(q: { readonly box: BoxProps }): JSX.Element {
  const props = q.box;
  return (
    <>
      <Show when={props.queue}>
        {(q) => (
          <WithAct
            act={q().act}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="c-queue" data-part="composer.queueSwitch" data-on={flag(q().on)} aria-pressed={q().on} title={q().title}>
                <Icon name="queue" />
                <span data-text="control">{q().label}</span>
                <i data-kit="c-switch" />
              </button>
            )}
          />
        )}
      </Show>
      <Show when={!props.plus}>
      <WithAct
        act={props.laterAct}
        render={(trigger) => (
          <button type="button" {...trigger()} data-kit="icon-btn" data-part="composer.sendLater" title={props.later} aria-label={props.later}>
            <Icon name="clock" />
          </button>
        )}
      />
      </Show>
    </>
  );
}

/** The phone looks (UX 2026-11-18 phone #1): the mode, the harness and the tools behind one chip and its sheet. */
export function ReplyPhoneOptions(q: { readonly box: BoxProps; readonly opts: () => (ReplyOptionLabels & { readonly goal?: string }) | undefined }): JSX.Element {
  const props = q.box;
  const phoneOpts = q.opts;
  return (
    <ReplyOptions
      labels={phoneOpts() as ReplyOptionLabels}
      mode={{ label: props.mode.label, icon: props.mode.icon, act: props.mode.act }}
      {...(props.runner && { runner: { title: props.runner.title, logo: props.runner.logo, model: props.runner.model, act: props.runner.act, open: props.runner.open } })}
      rows={[
        ...(props.plus
          ? []
          : [
              { id: 'team', part: 'composer.team', icon: 'agent' as const, label: props.teamAct?.panel?.label ?? props.team, act: props.teamAct },
              { id: 'goal', part: 'composer.goal', icon: 'target' as const, label: phoneOpts()?.goal ?? props.goal, on: props.goalOn === true, act: props.goalAct },
              { id: 'later', part: 'composer.sendLater', icon: 'clock' as const, label: props.later, act: props.laterAct },
            ]),
        ...(props.queue ? [{ id: 'queue', part: 'composer.queueSwitch', icon: 'queue' as const, label: props.queue.label, on: props.queue.on, act: props.queue.act }] : []),
      ]}
    />
  );
}
