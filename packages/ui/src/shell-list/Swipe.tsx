// Swipe: a touch row that runs an action when pulled sideways (v1 web/js/features/swipe.js). The band
// under the row says the action in the menus' words, warms with the pull and fills once letting go
// would run it. A side with no action only gives a little. Letting go past the line flies the row off
// its edge, closes its height, then calls onSwipe; anything short springs back. Touch only: a mouse
// or pen drag is a text selection. No timers: each step waits for the row's own animations to finish (none: at once).
// In the phone apps (ADR-0065 build step 4, design/mobile.md §A.4) Android draws the same band in Material 3
// (tonal, the icon leading, warming with --k-sw-p); iOS draws Mail's action instead: a button revealed from
// the edge, where a short pull lets the row rest open on it (tap it to run, tap the row to close) and a pull
// past half the row runs it at once. The words, the warming, the tick past the line and a running row's
// little give are the same on all three.
// Build step 7 (design/mobile.md §A.4–§A.6): under a phone look the row also answers a long-press (`onLongPress`:
// Android's bottom sheet of the row's actions with Select first, iOS's context menu with a preview: step 5's
// ContextMenu phone variants, opened by the caller on the row it's handed), carries `data-gesture`/`data-twin`, and gives a
// screen reader each side's action and the menu as custom actions. A swipe starting in a horizontal scroller inside
// the row (a chip row, code) scrolls it. The browser gets none of this: its HTML is what it was.
import { createSignal, onCleanup, Show, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useT } from '../core/i18n.tsx';
import { inScroller } from '../gestures/scroller.ts';
import { bindLater } from '../gestures/binders-part.ts';
import { useGestureHost } from '../gestures/host.tsx';
import { useGesture } from '../gestures/use.ts';
import { SWIPE_ACTION_W, swipeArmedFor, swipeOffset, swipePhase, swipeProgress, swipeRelease, swipeSpeed, swipeStarts, type SwipeStyle } from './swipe-math.ts';

export type SwipeDir = 'left' | 'right';

export interface SwipeSide {
  readonly label: string;
  readonly icon: IconName;
  readonly tone: 'good' | 'accent' | 'neutral' | 'bad';
}

export interface SwipeProps extends KitProps, WithChildren {
  /** what pulling the row to the left does; none: it only gives a little */
  readonly left?: SwipeSide | undefined;
  readonly right?: SwipeSide | undefined;
  readonly onSwipe: (dir: SwipeDir) => void;
  readonly disabled?: boolean;
  /** the pull crossed the line (true) or came back (false): a haptic tick */
  readonly onArm?: (armed: boolean) => void;
  /** the phone apps: a long-press opens the row's actions (its menu: a ContextMenu around the list opens on a
   * contextmenu dispatched on `row`, Android's sheet or iOS's context menu); also the screen reader's *Actions* */
  readonly onLongPress?: (row: HTMLElement) => void;
  /** which registered gesture the swipe is (its twin's parts, qa/platform.mjs): a task row's (default) or a saved thing's
   *  in My Stuff (`stuff.swipe`, twinned by the row's ⋯ menu). Additive (ADR-0065 step 14). */
  readonly gesture?: 'row.swipe' | 'stuff.swipe';
}

/** `open`: iOS only, the row rests on its revealed action */
type Phase = 'idle' | 'drag' | 'open' | 'gone' | 'closing';
/** controls inside the row keep their own touches */
const OWN = "[data-kit='button'], [data-kit='icon-button'], input, textarea, select, [contenteditable='true'], [data-swipe='none']";

interface Track {
  x: number;
  y: number;
  width: number;
  mode: 'pending' | 'swipe' | 'scroll';
  samples: { x: number; t: number }[];
}

export function Swipe(props: SwipeProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.swipe';
  const look = usePlatform();
  const style = (): SwipeStyle => (look() === 'ios' ? 'reveal' : 'band');
  const phone = (): boolean => look() !== 'web';
  const [phase, setPhase] = createSignal<Phase>('idle');
  const [dx, setDx] = createSignal(0);
  const [armed, setArmed] = createSignal(false);
  const [height, setHeight] = createSignal<number | undefined>(undefined);
  const [progress, setProgress] = createSignal(0);
  /** `run`: which fly the steps belong to (a reset or a new fly makes an older one's steps do nothing) */
  const st: { track: Track | undefined; swallow: boolean; dir: SwipeDir; width: number; el: HTMLDivElement | undefined; run: number } = { track: undefined, swallow: false, dir: 'right', width: 0, el: undefined, run: 0 };
  const side = (d: number): SwipeSide | undefined => (d > 0 ? props.right : d < 0 ? props.left : undefined);
  const host = useGestureHost();
  const t = useT();
  // the phone apps: the marks qa/platform.mjs reads and the screen reader's actions (the browser: nothing)
  const swipeMarks = useGesture(
    props.gesture ?? 'row.swipe',
    () => () => undefined,
    () => [
      ...(props.right && !props.disabled ? [{ label: props.right.label, run: () => props.onSwipe('right') }] : []),
      ...(props.left && !props.disabled ? [{ label: props.left.label, run: () => props.onSwipe('left') }] : []),
    ],
  );
  const pressMenu = useGesture(
    'row.long-press',
    (el, h) =>
      bindLater((b) =>
        b.bindLongPress(el, {
          host: h,
          onLongPress: () => {
            // the press is the menu's, not the start of a swipe
            st.track = undefined;
            props.onLongPress?.(el);
          },
        }),
      ),
    () => (props.onLongPress ? [{ label: t('gesture.actions'), run: () => st.el && props.onLongPress?.(st.el) }] : []),
  );

  // iOS (Mail): a row resting open closes on a touch anywhere else (another row, a scroll), so one rests open at most
  const outside = (e: PointerEvent): void => {
    if (st.el && e.target instanceof Node && st.el.contains(e.target)) return;
    reset();
  };
  const unlisten = (): void => document.removeEventListener('pointerdown', outside, true);
  onCleanup(unlisten);
  const reset = (): void => {
    unlisten();
    st.run += 1;
    st.track = undefined;
    setPhase('idle');
    setDx(0);
    setArmed(false);
    setHeight(undefined);
    setProgress(0);
  };
  /** fly the row off its edge, close its height, then run the action: each step once the row's own animations have
   *  finished, at once when there are none (audit A #8: waiting on a `transitionend` that never came, the row sat at
   *  `closing` and nothing ran) */
  const fly = (dir: SwipeDir, width: number): void => {
    unlisten();
    const run = (st.run += 1);
    st.dir = dir;
    setArmed(true);
    setPhase('gone');
    setProgress(1);
    setDx((dir === 'right' ? 1 : -1) * (width + 24));
    settled(st.el, () => {
      if (st.run !== run) return;
      setHeight(st.el?.getBoundingClientRect().height ?? 0);
      requestAnimationFrame(() => {
        if (st.run !== run) return;
        setPhase('closing');
        setHeight(0);
        settled(st.el, () => {
          if (st.run !== run) return;
          reset();
          props.onSwipe(dir);
        });
      });
    });
  };
  const down = (e: PointerEvent): void => {
    st.swallow = false;
    // iOS: a row resting open closes on any touch outside its action, and that touch opens nothing
    if (phase() === 'open') {
      if (e.target instanceof Element && e.target.closest("[data-kit='swipe-action']")) return;
      reset();
      st.swallow = true;
      return;
    }
    if (props.disabled || e.pointerType !== 'touch' || !e.isPrimary || phase() !== 'idle') return;
    if (e.target instanceof Element && e.target.closest(OWN)) return;
    if (!swipeStarts(e.clientX, window.innerWidth)) return;
    // a chip row or code inside the row scrolls; it never swipes the row
    if (st.el && inScroller(e.target, st.el)) return;
    st.track = { x: e.clientX, y: e.clientY, width: st.el?.getBoundingClientRect().width ?? 0, mode: 'pending', samples: [{ x: e.clientX, t: e.timeStamp }] };
  };
  const move = (e: PointerEvent): void => {
    const tr = st.track;
    if (!tr || tr.mode === 'scroll') return;
    const ddx = e.clientX - tr.x;
    if (tr.mode === 'pending') {
      tr.mode = swipePhase(ddx, e.clientY - tr.y);
      if (tr.mode !== 'swipe') return;
      st.el?.setPointerCapture(e.pointerId);
      setPhase('drag');
    }
    tr.samples.push({ x: e.clientX, t: e.timeStamp });
    if (tr.samples.length > 12) tr.samples.shift();
    const can = side(ddx) !== undefined;
    setDx(swipeOffset(ddx, can));
    if (phone()) setProgress(swipeProgress(style(), ddx, tr.width, can));
    const now = swipeArmedFor(style(), ddx, tr.width, can);
    if (now !== armed()) {
      setArmed(now);
      if (props.onArm) props.onArm(now);
      else if (now) host.haptic('select');
    }
  };
  const up = (e: PointerEvent): void => {
    const tr = st.track;
    st.track = undefined;
    if (!tr || tr.mode !== 'swipe') return;
    st.swallow = true;
    const ddx = e.clientX - tr.x;
    const can = side(ddx) !== undefined;
    const release = swipeRelease(style(), ddx, tr.width, can, swipeSpeed(tr.samples));
    if (release === 'close') return reset();
    st.dir = ddx > 0 ? 'right' : 'left';
    st.width = tr.width;
    if (release === 'commit') return fly(st.dir, tr.width);
    // iOS: rest open on the action, the band at its full warmth short of the line
    const rest = openWidth();
    setArmed(false);
    setPhase('open');
    document.addEventListener('pointerdown', outside, true);
    setDx(Math.sign(ddx) * rest);
    setProgress(swipeProgress(style(), rest, tr.width, true));
  };
  /** iOS: the resting width: Mail's 74 pt, or the action's words in full when they need more (one layout
   *  read, on release; the words never wrap and never truncate at rest) */
  const openWidth = (): number => {
    const b = st.el?.querySelector<HTMLElement>("[data-kit='swipe-action']");
    const l = b?.querySelector<HTMLElement>("[data-kit='swipe-label']");
    if (!b || !l) return SWIPE_ACTION_W;
    const cs = getComputedStyle(b);
    // scrollWidth rounds to whole px: one more keeps the last glyph off the ellipsis
    const natural = Math.ceil(l.scrollWidth + 1 + (Number.parseFloat(cs.paddingLeft) || 0) + (Number.parseFloat(cs.paddingRight) || 0));
    return Math.max(SWIPE_ACTION_W, Math.min(natural, st.width / 2));
  };
  /** iOS: the revealed action, tapped */
  const runOpen = (e: MouseEvent): void => {
    e.stopPropagation();
    if (phase() === 'open') fly(st.dir, st.width);
  };
  const click = (e: MouseEvent): void => {
    if (!st.swallow) return;
    st.swallow = false;
    e.preventDefault();
    e.stopPropagation();
  };
  const band = (): SwipeSide | undefined => side(dx());

  return (
    <div
      ref={(el) => {
        st.el = el;
        swipeMarks(el);
        if (props.onLongPress) pressMenu(el);
      }}
      id={props.id}
      data-kit="swipe"
      data-part={part()}
      data-phase={phase()}
      data-armed={flag(armed())}
      data-dir={dx() > 0 ? 'right' : dx() < 0 ? 'left' : undefined}
      style={{
        '--k-sw-x': `${dx()}px`,
        '--k-sw-h': height() === undefined ? undefined : `${height()}px`,
        // the phone looks only: how warm the band is (0…1 at the line) and how wide the revealed part is
        '--k-sw-p': phone() ? String(Math.round(Math.min(progress(), 1.5) * 1000) / 1000) : undefined,
        '--k-sw-w': phone() ? `${Math.abs(dx())}px` : undefined,
      }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={reset}
      onContextMenu={() => phase() === 'drag' && reset()}
      on:click={{ handleEvent: click, capture: true }}
    >
      <Show when={band()}>
        {(b) => (
          <Show
            when={style() === 'reveal'}
            fallback={
              <div data-kit="swipe-band" data-part={sub(part(), 'band')} data-tone={b().tone} aria-hidden="true">
                <Icon name={armed() ? 'check' : b().icon} size="md" />
                <span data-kit="swipe-label" data-text="control">{b().label}</span>
              </div>
            }
          >
            {/* iOS (Mail): the action is a button in the revealed part; the row's own accessibility actions name it (step 7) */}
            <div data-kit="swipe-band" data-part={sub(part(), 'band')} data-tone={b().tone} aria-hidden="true">
              <button type="button" tabIndex={-1} data-kit="swipe-action" data-part={sub(part(), 'action')} onClick={runOpen}>
                <Icon name={b().icon} size="md" />
                <span data-kit="swipe-label" data-text="control">{b().label}</span>
              </button>
            </div>
          </Show>
        )}
      </Show>
      <div data-kit="swipe-row" data-part={sub(part(), 'row')}>
        {props.children}
      </div>
    </div>
  );
}

/** `then`, once what `el` and its children are animating (the fly's transform, the close's height) has finished, from the
 * next frame (the step's styles applied); at once when nothing is (no transition on this look, reduced motion, a test
 * page). Finished or cancelled alike: the action must run. */
function settled(el: HTMLElement | undefined, then: () => void): void {
  requestAnimationFrame(() => {
    const running = (el?.getAnimations?.({ subtree: true }) ?? []).filter((a) => a.playState === 'running');
    if (running.length === 0) return then();
    void Promise.allSettled(running.map((a) => a.finished)).then(then);
  });
}
