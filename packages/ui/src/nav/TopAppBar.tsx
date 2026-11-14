// TopAppBar: a screen's bar (design/mobile.md §A.1, §A.7). `kind` says where the screen sits, the kit
// picks the platform's bar:
//   root (a destination's list)   Android: M3 small top app bar, one-line title.
//                                 iOS: navigation bar with a large title that collapses to the inline title.
//   page (a pushed task, setting) Android: M3 medium top app bar, the title on up to two lines under the
//                                 bar, collapsing into the bar on scroll.
//                                 iOS: the inline, centred title; Back carries the previous title.
// Actions are a Toolbar (label → icon → More by priority; More is ⋮ on Android, ⋯ on iOS). On iOS a root
// bar also draws the compose button a Fab handed over (compose.tsx). In the browser it is a plain one-line
// bar from the browser kit (Back, title, actions), so it needs no CSS of its own there.
//
// In the apps the screen's bar is the Frame's `top`. The collapse follows the scroll of the frame around
// it, or of the box it sits in (a capturing passive listener, like Fab): the
// large title's height is read once on mount, then one scrollTop read per event and one attribute write
// when the state flips (scroll-math.ts barCollapsed).
import { createSignal, onCleanup, onMount, Show, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { useT, type Translate } from '../core/i18n.tsx';
import { Button, IconButton } from '../controls/Button.tsx';
import { Icon } from '../content/Icon.tsx';
import { Text } from '../content/Text.tsx';
import { Inline } from '../layout/Stack.tsx';
import { Box } from '../layout/Box.tsx';
import { Toolbar, type ToolbarItem } from '../composites/Toolbar.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { ComposeButton, claimCompose, useComposeHost } from './compose.tsx';
import { barCollapsed } from './scroll-math.ts';
import { useTaskSwipe, type TaskSwipe } from '../gestures/use.ts';

export interface TopAppBarBack {
  /** What Back returns to (iOS shows it, shortened to the kit's "Back" when long; Android names the button). */
  readonly label: string;
  readonly onBack: () => void;
}

export interface TopAppBarProps extends KitProps {
  readonly title: string;
  readonly kind?: 'root' | 'page';
  readonly back?: TopAppBarBack;
  /** The title as a button with a chevron (the computer filter on the list). */
  readonly onTitle?: () => void;
  readonly actions?: readonly ToolbarItem[];
  /** The actions toolbar's name (default: the title). */
  readonly actionsLabel?: string;
  readonly t?: Translate;
  /** A task page (the phone apps): swiping the head left / right opens the next / previous task in the list's order
   *  (design/mobile.md §A.4); the twins are Next task / Previous task in `actions`' menu. The browser binds nothing. */
  readonly swipe?: TaskSwipe;
}

/** iOS shortens the back title the way UINavigationBar does when it doesn't fit: past this many characters, "Back". */
export const BACK_TITLE_MAX = 12;

export function backTitle(label: string, t: Translate): string {
  return [...label].length > BACK_TITLE_MAX ? t('back') : label;
}

function Title(props: { readonly title: string; readonly onTitle: (() => void) | undefined; readonly part: PartName }): JSX.Element {
  return (
    <Show when={props.onTitle} fallback={<h1 data-kit="app-bar-title" data-part={props.part} data-text="control">{props.title}</h1>}>
      {(go) => (
        <h1 data-kit="app-bar-title" data-part={props.part}>
          <button type="button" data-kit="app-bar-title-button" onClick={() => go()()}>
            <span data-text="control">{props.title}</span>
            <Icon name="chevron" size="sm" />
          </button>
        </h1>
      )}
    </Show>
  );
}

function WebBar(props: TopAppBarProps & { readonly part: PartName }): JSX.Element {
  return (
    <Inline as="header" gap="4" align="center" {...(props.id === undefined ? {} : { id: props.id })} part={props.part}>
      <Show when={props.back}>{(b) => <IconButton icon="back" label={b().label} onClick={() => b().onBack()} part={sub(props.part, 'back')} />}</Show>
      <Text weight="semibold" truncate part={sub(props.part, 'title')}>
        {props.title}
      </Text>
      <Box grow>
        <Show when={props.actions?.length}>
          <Toolbar aria-label={props.actionsLabel ?? props.title} items={props.actions ?? []} justify="end" part={sub(props.part, 'actions')} />
        </Show>
      </Box>
    </Inline>
  );
}

function PhoneBar(props: TopAppBarProps & { readonly part: PartName }): JSX.Element {
  const t = useT(() => props.t);
  const look = usePlatform();
  const kind = (): 'root' | 'page' => props.kind ?? 'page';
  const ios = (): boolean => look() === 'ios';
  // M3 medium on a pushed page; the iOS large title on a destination's root
  const large = (): boolean => (ios() ? kind() === 'root' : kind() === 'page');
  const host = useComposeHost();
  claimCompose(host, ios() && kind() === 'root');
  const [collapsed, setCollapsed] = createSignal(false);
  const swipe = useTaskSwipe(() => props.swipe ?? {});
  let el: HTMLElement | undefined;
  let largeEl: HTMLDivElement | undefined;
  onMount(() => {
    // the frame around it (its panes scroll under it), else the box it sits in
    const box = el?.closest('[data-kit="frame"]') ?? el?.parentElement;
    if (!box || !largeEl) return;
    const h = largeEl.offsetHeight;
    const onScroll = (e: Event): void => {
      const tg = e.target;
      if (!(tg instanceof Element) || tg.scrollHeight <= tg.clientHeight) return;
      const next = barCollapsed(collapsed(), tg.scrollTop, h);
      if (next !== collapsed()) setCollapsed(next);
    };
    box.addEventListener('scroll', onScroll, { capture: true, passive: true });
    onCleanup(() => box.removeEventListener('scroll', onScroll, { capture: true }));
  });
  return (
    <header
      ref={(h) => {
        el = h;
        if (props.swipe) swipe(h);
      }}
      id={props.id}
      data-kit="app-bar" data-part={props.part} data-kind={kind()} data-large={flag(large())} data-collapsed={flag(large() && collapsed())}>
      <div data-kit="app-bar-row">
        <Show when={props.back}>
          {(b) => (
            <Show when={ios()} fallback={<IconButton icon="back" label={b().label} onClick={() => b().onBack()} part={sub(props.part, 'back')} />}>
              <Button variant="ghost" icon="back" aria-label={b().label} onClick={() => b().onBack()} part={sub(props.part, 'back')}>
                {backTitle(b().label, t)}
              </Button>
            </Show>
          )}
        </Show>
        <Title title={props.title} onTitle={props.onTitle} part={sub(props.part, 'title')} />
        <div data-kit="app-bar-actions">
          <Show when={props.actions?.length}>
            <Toolbar aria-label={props.actionsLabel ?? props.title} items={props.actions ?? []} justify="end" part={sub(props.part, 'actions')} t={t} />
          </Show>
          <ComposeButton when={ios() && kind() === 'root'} />
        </div>
      </div>
      <Show when={large()}>
        <div ref={largeEl} data-kit="app-bar-large">
          <Show when={props.onTitle} fallback={<span data-kit="app-bar-large-title" aria-hidden="true">{props.title}</span>}>
            {(go) => (
              <button type="button" data-kit="app-bar-title-button" data-large="" aria-hidden="true" tabIndex={-1} onClick={() => go()()}>
                <span data-kit="app-bar-large-title">{props.title}</span>
                <Icon name="chevron" size="md" />
              </button>
            )}
          </Show>
        </div>
      </Show>
    </header>
  );
}

export function TopAppBar(props: TopAppBarProps): JSX.Element {
  const look = usePlatform();
  const part = (): PartName => props.part ?? 'ui.app-bar';
  return (
    <Show when={look() !== 'web'} fallback={<WebBar {...props} part={part()} />}>
      <PhoneBar {...props} part={part()} />
    </Show>
  );
}
