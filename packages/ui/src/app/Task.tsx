// The open task as v1 draws it in the main pane (web/css/split.css .split-head, panel.css .it-body,
// .it-bar, .it-actions, transcript.css .turn/.ask/.trail, ship.css .overlap-strip, composer.css):
// its head, then its turns with the details column beside them, and the reply box docked at the foot.
import { Show, createSignal, onCleanup, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { holdMedia, type Held } from './hold-view.ts';
import { createTimer } from '../blocks/content-scope.ts';
import { IconButton } from '../controls/Button.tsx';
import '../css/transcript.css';
import '../css/app-usage.css';
import '../css/rate.css';
export * from './TaskHead.tsx';
export * from './TaskDetails.tsx';
export * from './TurnView.tsx';
export * from './OverlapNote.tsx';
export * from './reply-box-props.ts';
export * from './ReplyBox.tsx';

/** Imperative scroll control exposed by TaskBody so the host (TaskView) coordinates follow-ups, initial landing and
 *  kept-place restore with hold-view's follow state instead of writing raw scrollTop behind its back. */
export type TaskScroll = Pick<Held, 'toEnd' | 'place' | 'following'>;

/** v1 .it-body: the task's page under its head: the details column, the turns, the dock. */
/** v1 term.css .tui-term (Babysitting mode): what takes the task body's place edge to edge (a terminal task's CLI), under
 *  its head, in place of the turns, details and reply box. Nothing around it. Added 2026-11-25 (additive). */
export function TaskScreen(props: { readonly children: JSX.Element }): JSX.Element {
  return (
    <div data-kit="task-screen" data-part="task.screen">
      {props.children}
    </div>
  );
}

export function TaskBody(props: {
  readonly details?: JSX.Element;
  readonly turns: JSX.Element;
  readonly dock: JSX.Element;
  readonly wide?: boolean;
  /** Settings → Chat → Chat width. Additive. */
  readonly chatWidth?: 'comfortable' | 'wide' | 'full';
  /** Settings → Chat → Chat mode. Additive. */
  readonly chat?: boolean;
  /** the scroller (a task's kept place) */ readonly bodyRef?: (el: HTMLElement) => void;
  /** coordinated scroll controller (follow-ups, place restore, Chat initial landing). Additive. */
  readonly scrollRef?: (ctrl: TaskScroll) => void;
  /** Jump to latest's name: the button over the dock once the reader scrolled away from the end. Additive. */
  readonly toLatest?: string;
}): JSX.Element {
  let stop: (() => void) | undefined;
  let held: Held | undefined;
  let body!: HTMLDivElement;
  onCleanup(() => stop?.());
  // away from the end (the view no longer follows) and whether more came in since: hold-view says when it changes
  const [away, setAway] = createSignal({ away: false, fresh: false });
  // shown 150 ms after the reader left the end (t3code), gone at once when they're back; updates to `fresh` during the
  // delay piggyback on the running timer instead of restarting it
  const later = createTimer();
  let pendingAway: { readonly away: boolean; readonly fresh: boolean } | undefined;
  const onAway = (o: { readonly away: boolean; readonly fresh: boolean }): void => {
    if (o.away && !away().away) {
      const waiting = pendingAway !== undefined;
      pendingAway = o;
      if (!waiting) {
        later.after(150, () => {
          const next = pendingAway;
          pendingAway = undefined;
          if (next) setAway(next);
        });
      }
      return;
    }
    pendingAway = undefined;
    later.clear();
    setAway(o);
  };
  // at once, not smooth: a smooth scroll's steps read as the reader leaving the end again (and the click answers in
  // the frame, ADR-0017)
  const toLatest = (): void => {
    pendingAway = undefined;
    later.clear();
    setAway({ away: false, fresh: false });
    if (held) held.toEnd();
    else body.scrollTop = body.scrollHeight;
  };
  return (
    <div
      ref={(el) => {
        body = el;
        props.bodyRef?.(el);
      }}
      data-kit="it-body"
      data-part="split.body card.body"
      data-details={flag(props.details !== undefined)}
      data-width={props.chatWidth}
      data-chat={flag(props.chat)}
    >
      {props.details}
      <div data-kit="turns">{props.turns}</div>
      <div
        ref={(dock) => {
          // a child's ref runs after its parent's: the body is known here
          stop?.();
          held = typeof ResizeObserver === 'undefined' ? undefined : holdMedia(body, dock, onAway);
          if (held) props.scrollRef?.({ toEnd: held.toEnd, place: held.place, following: held.following });
          const h = held;
          stop = () => h?.stop();
        }}
        data-kit="it-dock"
      >
        <Show when={props.toLatest !== undefined && away().away}>
          <span data-kit="to-latest" data-part="task.toLatest" data-fresh={flag(away().fresh)}>
            <IconButton icon="arrow-down" variant="secondary" label={props.toLatest ?? ''} onClick={toLatest} />
          </span>
        </Show>
        {props.dock}
      </div>
    </div>
  );
}

