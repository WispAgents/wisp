// Fab: the screen's one main action (design/mobile.md §A.1, §A.7), New task over the list, New job on Crew.
// Android: the M3 extended FAB (56 dp, 16 dp corners), shrinking to the 56 dp FAB while its pane scrolls
// down and growing back on scroll up (scroll-math.ts), above the navigation bar and the gesture bar. It
// gives way to a row's question choices (v1 fab.js giveWay): it fades and sinks while one is on screen,
// and comes back. iOS: not drawn; the action becomes the navigation bar's trailing compose button
// (compose.tsx). The browser: v1's round floating button (v1 fab.js, home.css), unchanged.
//
// The pane's scroll is heard where it happens: a capturing, passive listener on the pane around the Fab,
// so any scroller inside (a VirtualList) drives it with no prop. One read of scrollTop per event, no write
// but the one attribute when the state flips.
import { createSignal, Match, onCleanup, onMount, Show, Switch, type JSX } from 'solid-js';
import { flag, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useComposeHost } from './compose.tsx';
import { FAB_START, fabOnScroll } from './scroll-math.ts';
import { watchGiveWay } from './give-way.ts';

export interface FabProps extends KitProps {
  /** The action's name: the extended FAB's label, the small one's and the iOS compose button's accessible name. */
  readonly label: string;
  /** On iOS the compose button uses the `compose` glyph instead. */
  readonly icon: IconName;
  readonly onClick: () => void;
  /** Something with choices is on screen under it: it fades and sinks out of reach, and comes back. */
  readonly giveWay?: boolean;
  /** Android: the small FAB (icon only) for good. Default: extended, shrinking on scroll. */
  readonly small?: boolean;
  /** Give way while an element matching this is on screen in the pane (v1: a row's question choices). */
  readonly giveWayTo?: string;
}

/** The pane (or the nearest positioned box) whose scroll and contents the FAB follows. */
function paneOf(el: HTMLElement): HTMLElement | null {
  return el.closest<HTMLElement>('[data-kit="frame-pane"]') ?? el.parentElement;
}

function AndroidFab(props: FabProps): JSX.Element {
  const [scroll, setScroll] = createSignal(FAB_START);
  const [watchedAway, setWatchedAway] = createSignal(false);
  let el: HTMLButtonElement | undefined;
  onMount(() => {
    const pane = el ? paneOf(el) : null;
    if (!pane) return;
    const onScroll = (e: Event): void => {
      const t = e.target;
      // the document scrolling, or a scroller inside the pane (a sideways strip doesn't count)
      const y = t instanceof Element ? t.scrollTop : (document.scrollingElement?.scrollTop ?? 0);
      if (t instanceof Element && t.scrollHeight <= t.clientHeight) return;
      const next = fabOnScroll(scroll(), y);
      if (next.extended !== scroll().extended || next.anchor !== scroll().anchor) setScroll(next);
    };
    pane.addEventListener('scroll', onScroll, { capture: true, passive: true });
    onCleanup(() => pane.removeEventListener('scroll', onScroll, { capture: true }));
    const sel = props.giveWayTo;
    if (sel) onCleanup(watchGiveWay(pane, sel, setWatchedAway));
  });
  const extended = (): boolean => !props.small && scroll().extended;
  const away = (): boolean => !!props.giveWay || watchedAway();
  return (
    <button
      ref={el}
      id={props.id}
      type="button"
      data-kit="fab"
      data-part={props.part ?? 'ui.fab'}
      data-extended={flag(extended())}
      data-give-way={flag(away())}
      aria-label={props.label}
      aria-hidden={away() ? 'true' : undefined}
      tabIndex={away() ? -1 : undefined}
      onClick={() => props.onClick()}
    >
      <Icon name={props.icon} />
      <Show when={!props.small}>
        <span data-kit="fab-label" data-text="control" aria-hidden="true">
          {props.label}
        </span>
      </Show>
    </button>
  );
}

/** The browser's (v1 fab.js): a round button, its label as name and tooltip. */
function WebFab(props: FabProps): JSX.Element {
  return (
    <button
      type="button"
      id={props.id}
      data-kit="fab"
      data-part={props.part ?? 'ui.fab'}
      data-give-way={flag(props.giveWay)}
      aria-hidden={props.giveWay ? 'true' : undefined}
      tabIndex={props.giveWay ? -1 : 0}
      aria-label={props.label}
      title={props.label}
      onClick={() => props.onClick()}
    >
      <Icon name={props.icon} size="lg" />
    </button>
  );
}

function IosCompose(props: FabProps): JSX.Element {
  const host = useComposeHost();
  const part: PartName = props.part ?? 'ui.fab';
  if (host)
    onCleanup(
      host.offer({
        get label() {
          return props.label;
        },
        icon: 'compose',
        onSelect: () => props.onClick(),
        part,
      }),
    );
  return null;
}

export function Fab(props: FabProps): JSX.Element {
  const look = usePlatform();
  return (
    <Switch fallback={<WebFab {...props} />}>
      <Match when={look() === 'android'}>
        <AndroidFab {...props} />
      </Match>
      <Match when={look() === 'ios'}>
        <IosCompose {...props} />
      </Match>
    </Switch>
  );
}
