// The crew on phones as stories (ADR-0065 step 13b, docs/design/crew-stories.html direction C, the owner's pick): a
// row of rings under the Tasks top bar (Org first, a ring per member, + last) and a member's story, full screen. The
// ring says what a member is up to: working (green, moving), a report you haven't read (accent), waiting on you
// (red: an ask or an approval), idle (plain). The story steps through the crew like stories do: it moves on by itself
// after a while (the bar at the top fills), a press held anywhere pauses it, a tap on the left or right half steps
// back or on, a drag down closes it. Every gesture has its control on screen: ← and → (and the arrow keys), Pause /
// Resume, ×. Only the phone apps draw it (the crew plugin's phone chunk); the browser keeps v1's Crew section.
import { For, Show, createMemo, createSignal, onCleanup, type JSX } from 'solid-js';
import { flag, type KitProps } from '../core/props.ts';
import type { Translate } from '../core/i18n.tsx';
import { Icon } from '../content/Icon.tsx';
import { Dialog } from '../controls/Dialog.tsx';
import { IconButton } from '../controls/Button.tsx';
import { Menu } from '../controls/Menu.tsx';
import type { MenuEntry } from '../controls/menu-entries.ts';
import { owns } from '../gestures/bind.ts';
import { bindSheetDrag } from '../gestures/bind-vertical.ts';
import { GESTURE_SLOP, LONG_PRESS_MS } from '../gestures/gesture-math.ts';
import { useGestureHost } from '../gestures/host.tsx';
import { gestureMarks } from '../gestures/registry.ts';
import { Face } from './Face.tsx';
import type { SpiritMood } from './spirit-code.ts';
import '../css/crew-stories.css';

/** What a member's ring says: waiting on you, a report you haven't read, working, idle. */
export type StoryRing = 'wait' | 'report' | 'work' | 'idle';

export interface StoryBubble {
  readonly id: string;
  readonly name: string;
  readonly avatar: string;
  readonly color?: string | undefined;
  readonly ring: StoryRing;
  /** its accessible name: "Ada, waiting on you" */
  readonly label: string;
  /** paused: dimmed */
  readonly dim?: boolean | undefined;
}

export interface CrewStoriesProps extends KitProps {
  /** the row's accessible name ("Crew") */
  readonly label: string;
  readonly members: readonly StoryBubble[];
  readonly onOpen: (id: string) => void;
  /** Org, the first bubble (none until there's a crew) */
  readonly org?: { readonly label: string; readonly title?: string; readonly onClick: () => void } | undefined;
  /** Add a member, the last bubble */
  readonly add: { readonly label: string; readonly title?: string; readonly onClick: () => void };
}

const MOOD: Readonly<Record<StoryRing, SpiritMood>> = { wait: 'asks', report: 'idle', work: 'live', idle: 'idle' };

function Bubble(p: { readonly ring: StoryRing | 'org' | 'add'; readonly name: string; readonly label: string; readonly part: string; readonly onClick: () => void; readonly children: JSX.Element }): JSX.Element {
  return (
    <div role="listitem" data-kit="story-item">
      <button type="button" data-kit="story-bubble" data-ring={p.ring} data-part={p.part} aria-label={p.label} title={p.label} onClick={() => p.onClick()}>
        <span data-kit="story-ring">{p.children}</span>
        <span data-kit="story-name" data-text="control">
          {p.name}
        </span>
      </button>
    </div>
  );
}

/** The stories row: Org, the members in the order given (the caller ranks them), Add. It scrolls sideways. */
export function CrewStories(props: CrewStoriesProps): JSX.Element {
  return (
    <div data-kit="stories" data-part={props.part ?? 'crew.stories'} role="list" aria-label={props.label}>
      <Show when={props.org}>
        {(o) => (
          <Bubble ring="org" name={o().label} label={o().title ?? o().label} part="crew.stories.org" onClick={o().onClick}>
            <span data-kit="story-glyph">
              <Icon name="org" />
            </span>
          </Bubble>
        )}
      </Show>
      <For each={props.members}>
        {(m) => (
          <Bubble ring={m.ring} name={m.name} label={m.label} part="crew.stories.member" onClick={() => props.onOpen(m.id)}>
            <Face avatar={m.avatar} name={m.name} {...(m.color && { color: m.color })} size="lg" mood={MOOD[m.ring]} {...(m.dim && { dim: true })} />
          </Bubble>
        )}
      </For>
      <Bubble ring="add" name={props.add.label} label={props.add.title ?? props.add.label} part="crew.stories.add" onClick={props.add.onClick}>
        <span data-kit="story-glyph">
          <Icon name="plus" />
        </span>
      </Bubble>
    </div>
  );
}

/** How long a member's story shows before the next (the bar fills over it). */
export const STORY_DWELL_MS = 8000;

export interface CrewStoryLabels {
  readonly close: string;
  readonly prev: string;
  readonly next: string;
  readonly pause: string;
  readonly resume: string;
  /** under the story, between the arrows: "4 of 6 · swipe down to close" */
  readonly where: string;
}

export interface CrewStoryProps {
  /** the dialog's accessible name ("Ada's story") */
  readonly title: string;
  readonly index: number;
  readonly count: number;
  /** go to the story at `to` (0 … count - 1) */
  readonly onStep: (to: number) => void;
  readonly onClose: () => void;
  readonly face: JSX.Element;
  readonly name: string;
  /** under the name: role · current job */
  readonly sub: string;
  /** the member's actions (⋯) */
  readonly menu?: { readonly label: string; readonly items: readonly MenuEntry[]; readonly onSelect: (value: string) => void } | undefined;
  readonly labels: CrewStoryLabels;
  /** the cards (Waiting for you, Now, Last report, Next) */
  readonly children: JSX.Element;
  /** the reply box at the bottom; it holds the story still while it has the caret */
  readonly reply?: JSX.Element;
  /** ms before the next member (STORY_DWELL_MS); 0: it never moves on by itself (and never under reduced motion) */
  readonly dwell?: number;
  /** held still by the caller (its own dialog over the story) */
  readonly hold?: boolean;
  readonly t?: Translate;
}

const TYPING = "input, textarea, select, [contenteditable='true'], [contenteditable='plaintext-only']";

const reduced = (): boolean => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

// size-ok: one surface whose gestures, keys and controls share its pause and its step
export function CrewStory(props: CrewStoryProps): JSX.Element {
  const host = useGestureHost();
  const [held, setHeld] = createSignal(false);
  const [pinned, setPinned] = createSignal(false);
  const [typing, setTyping] = createSignal(false);
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [drag, setDrag] = createSignal(0);
  const paused = (): boolean => held() || pinned() || typing() || menuOpen() || !!props.hold;
  // reduced motion runs every animation at once (motion.css): the story would race through the crew, so it waits
  const dwell = createMemo(() => (reduced() ? 0 : (props.dwell ?? STORY_DWELL_MS)));
  const bars = createMemo(() => Array.from({ length: props.count }, (_, i) => i));
  const step = (by: -1 | 1): void => {
    const to = props.index + by;
    if (to >= 0 && to < props.count) props.onStep(to);
  };
  const keys = (e: KeyboardEvent): void => {
    // the arrows are the story's unless the caret is in a field (its buttons don't take them)
    if (e.defaultPrevented || (e.target instanceof Element && e.target.closest(TYPING) !== null)) return;
    if (e.key === 'ArrowRight') step(1);
    else if (e.key === 'ArrowLeft') step(-1);
    else return;
    e.preventDefault();
  };
  // a press on the story (not on a control): held, it pauses; let go quickly where it began, a tap steps back (left
  // half) or on (right half)
  let press: { x: number; y: number; at: number } | undefined;
  const down = (e: PointerEvent): void => {
    press = undefined;
    if (!e.isPrimary || e.button !== 0 || owns(e.target)) return;
    press = { x: e.clientX, y: e.clientY, at: e.timeStamp };
    setHeld(true);
  };
  const move = (e: PointerEvent): void => {
    if (press && Math.hypot(e.clientX - press.x, e.clientY - press.y) > GESTURE_SLOP) press = undefined;
  };
  const up = (e: PointerEvent): void => {
    const p = press;
    press = undefined;
    setHeld(false);
    if (!p || e.timeStamp - p.at >= LONG_PRESS_MS) return;
    const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
    step(e.clientX < box.left + box.width / 2 ? -1 : 1);
  };
  const cancel = (): void => {
    press = undefined;
    setHeld(false);
  };
  let stop: (() => void) | undefined;
  onCleanup(() => stop?.());
  let body: HTMLElement | undefined;
  const bind = (el: HTMLElement): void => {
    stop?.();
    stop = bindSheetDrag(el, { host, onClose: () => props.onClose(), onDrag: (dy) => setDrag(dy), content: () => body });
  };
  return (
    <Dialog open bare size="full" title={props.title} part="crew.stories.view" closeOnOutside={false} onOpenChange={(open) => !open && props.onClose()} vars={{ '--k-story-dwell': `${dwell()}ms`, '--k-story-drag': `${drag()}px` }} {...(props.t && { t: props.t })}>
      <div
        data-kit="story"
        data-part="crew.story.page"
        data-paused={flag(paused())}
        data-dragging={flag(drag() > 0)}
        ref={bind}
        onKeyDown={keys}
        {...gestureMarks(['story.drag-down', 'story.long-press'])}
      >
        <div data-kit="story-progress" aria-hidden="true">
          <For each={bars()}>
            {(i) => (
              <i data-kit="story-bar" data-at={i < props.index ? 'done' : i === props.index ? 'now' : undefined}>
                <Show when={i === props.index}>
                  <b data-kit="story-fill" data-still={flag(dwell() === 0)} onAnimationEnd={() => step(1)} />
                </Show>
              </i>
            )}
          </For>
        </div>
        <header data-kit="story-head" data-part="crew.story.head">
          {props.face}
          <span data-kit="story-who">
            <b data-text="control">{props.name}</b>
            <span data-text="control">{props.sub}</span>
          </span>
          <Show when={dwell() > 0}>
            <IconButton
              icon={pinned() ? 'play' : 'pause'}
              label={pinned() ? props.labels.resume : props.labels.pause}
              pressed={pinned()}
              variant="ghost"
              size="sm"
              part="crew.story.pause"
              onClick={() => setPinned(!pinned())}
            />
          </Show>
          <Show when={props.menu}>
            {(m) => (
              <Menu
                part="crew.story.more"
                trigger={{ icon: 'dots', 'aria-label': m().label, variant: 'ghost', size: 'sm' }}
                items={m().items}
                onSelect={m().onSelect}
                onOpenChange={setMenuOpen}
                placement="bottom-end"
              />
            )}
          </Show>
          <IconButton icon="x" label={props.labels.close} variant="ghost" size="sm" part="crew.story.close" onClick={() => props.onClose()} />
        </header>
        <div data-kit="story-body" data-part="crew.story.body" ref={(el) => (body = el)} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel} onPointerLeave={cancel}>
          {props.children}
        </div>
        {/* `reply` is read once: a JSX prop is a getter, and reading it twice would build a second box */}
        {((reply) =>
          reply === undefined ? null : (
            <div data-kit="story-reply" data-part="crew.story.reply" onFocusIn={() => setTyping(true)} onFocusOut={() => setTyping(false)}>
              {reply}
            </div>
          ))(props.reply)}
        <nav data-kit="story-nav" data-part="crew.story.nav">
          <IconButton icon="back" label={props.labels.prev} variant="secondary" disabled={props.index <= 0} part="crew.story.prev" onClick={() => step(-1)} />
          <span data-kit="story-where" data-text="control">
            {props.labels.where}
          </span>
          <IconButton icon="arrow" label={props.labels.next} variant="secondary" disabled={props.index >= props.count - 1} part="crew.story.next" onClick={() => step(1)} />
        </nav>
      </div>
    </Dialog>
  );
}

/** One of a story's cards (Waiting for you, Now, Last report, Next): its label, then what it says. */
export function StoryCard(props: { readonly label: string; readonly tone?: 'bad' | undefined; readonly part?: string; readonly children: JSX.Element }): JSX.Element {
  return (
    <section data-kit="story-card" data-tone={props.tone} data-part={props.part ?? 'crew.story.card'} aria-label={props.label}>
      <h3 data-kit="story-label">{props.label}</h3>
      {props.children}
    </section>
  );
}

/** A card's words: its heading on one line, a line under it (live: the green dot of a running job). */
export function StoryText(props: { readonly title?: string | undefined; readonly line?: string | undefined; readonly live?: boolean }): JSX.Element {
  return (
    <>
      <Show when={props.title}>{(t) => <p data-kit="story-title">{t()}</p>}</Show>
      <Show when={props.line}>
        {(l) => (
          <p data-kit="story-line" data-live={flag(props.live)}>
            {l()}
          </p>
        )}
      </Show>
    </>
  );
}
