// TopAppBar: a screen's bar (design/mobile.md §A.1, §A.7). `kind` says where the screen sits, the kit
// picks the platform's bar:
//   root (a destination's list)   Android: M3 small top app bar, one-line title.
//                                 iOS: navigation bar with a large title that collapses to the inline title.
//   page (a pushed task, setting) Android: M3 medium top app bar, the title on up to two lines under the
//                                 bar, collapsing into the bar on scroll.
//                                 iOS: the inline, centred title; Back carries the previous title.
//   compose (New task)            Android: the M3 full-screen dialog's bar: Close (×), the title, the confirm
//                                 action as a text button at the end. iOS: a sheet's bar: Cancel, the centred
//                                 title, the confirm action in bold.
// `menu` is the page's ⋮ (Android) / ⋯ (iOS) menu at the end; `search` is the screen's search: an icon among the
// actions on Android (the M3 search view opens from it), the search field under the large title on iOS (an icon
// in the bar once the large title has folded away).
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
import { Menu, MenuOn, type MenuEntry } from '../controls/Menu.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { ComposeButton, claimCompose, useComposeHost } from './compose.tsx';
import { barCollapsed } from './scroll-math.ts';
import { useTaskSwipe, type TaskSwipe } from '../gestures/use.ts';

export interface TopAppBarBack {
  /** What Back returns to (iOS shows it, shortened to the kit's "Back" when long; Android names the button). */
  readonly label: string;
  readonly onBack: () => void;
  /** A compose bar's way out, with iOS's word for it (Cancel): Android draws × named `label`, iOS this word. Default:
   *  Back. */
  readonly close?: string;
}

/** A compose bar's confirm action (Send): Android's text button at the end, iOS's bold bar button. */
export interface TopAppBarConfirm {
  readonly label: string;
  readonly onSelect: () => void;
  readonly disabled?: boolean;
}

/** The page's menu (⋮ on Android, ⋯ on iOS), after its actions. */
export interface TopAppBarMenu {
  readonly label: string;
  readonly items: readonly MenuEntry[];
  readonly onSelect: (value: string) => void;
}

/** The screen's search: what opens it and its words (the iOS field's placeholder, the icon's name). */
export interface TopAppBarSearch {
  readonly label: string;
  readonly onOpen: () => void;
}

export interface TopAppBarProps extends KitProps {
  readonly title: string;
  readonly kind?: 'root' | 'page' | 'compose';
  readonly back?: TopAppBarBack;
  readonly confirm?: TopAppBarConfirm;
  readonly menu?: TopAppBarMenu;
  readonly search?: TopAppBarSearch;
  /** The title as a button with a chevron (the computer filter on the list). */
  readonly onTitle?: () => void;
  /** iOS: the title's button opens this pull-down menu (a UIMenu from the title, its choices with checkmarks) in place
   *  of `onTitle`, which Android and the browser keep. Additive (ADR-0065 step 13, 2026-11-15). */
  readonly titleMenu?: TopAppBarMenu;
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

function Title(props: { readonly title: string; readonly onTitle: (() => void) | undefined; readonly menu?: TopAppBarMenu | undefined; readonly part: PartName }): JSX.Element {
  return (
    <Show
      when={props.menu}
      fallback={
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
      }
    >
      {(m) => (
        <h1 data-kit="app-bar-title" data-part={props.part}>
          <TitleMenu menu={m()} part={sub(props.part, 'menu')}>
            {(trigger) => (
              <button {...trigger()} data-kit="app-bar-title-button" title={m().label}>
                <span data-text="control">{props.title}</span>
                <Icon name="chevron" size="sm" />
              </button>
            )}
          </TitleMenu>
        </h1>
      )}
    </Show>
  );
}

/** iOS's pull-down from the title (UIMenu from a navigation bar's title): the kit's menu under the title's button. */
function TitleMenu(props: { readonly menu: TopAppBarMenu; readonly part: PartName; readonly children: (trigger: () => Record<string, unknown>) => JSX.Element }): JSX.Element {
  return (
    <MenuOn items={props.menu.items} onSelect={(v) => props.menu.onSelect(v)} placement="bottom-start" part={props.part}>
      {props.children}
    </MenuOn>
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
      <Show when={props.search}>{(q) => <IconButton icon="search" label={q().label} onClick={() => q().onOpen()} part={sub(props.part, 'search')} />}</Show>
      <Show when={props.menu}>
        {(m) => <Menu trigger={{ icon: 'dots', 'aria-label': m().label }} items={m().items} onSelect={m().onSelect} placement="bottom-end" part={sub(props.part, 'menu')} />}
      </Show>
      <Show when={props.confirm}>
        {(c) => (
          <Button variant="primary" size="sm" disabled={c().disabled === true} onClick={() => c().onSelect()} part={sub(props.part, 'confirm')}>
            {c().label}
          </Button>
        )}
      </Show>
    </Inline>
  );
}

/** Back (a pushed page), or a compose bar's way out: Android's × / ←, iOS's titled Back or plain Cancel. */
function BackControl(props: { readonly back: TopAppBarBack; readonly ios: boolean; readonly part: PartName; readonly t: Translate }): JSX.Element {
  const go = (): void => props.back.onBack();
  return (
    <Show when={props.ios} fallback={<IconButton icon={props.back.close === undefined ? 'back' : 'x'} label={props.back.label} onClick={go} part={props.part} />}>
      <Show
        when={props.back.close}
        fallback={
          <Button variant="ghost" icon="back" aria-label={props.back.label} onClick={go} part={props.part}>
            {backTitle(props.back.label, props.t)}
          </Button>
        }
      >
        {(word) => (
          <Button variant="ghost" onClick={go} part={props.part}>
            {word()}
          </Button>
        )}
      </Show>
    </Show>
  );
}

/** The bar's end after its actions: the page's menu (⋮ on Android, ⋯ on iOS) and a compose bar's confirm (Send). */
function PageEnd(props: { readonly menu: TopAppBarMenu | undefined; readonly confirm: TopAppBarConfirm | undefined; readonly ios: boolean; readonly part: PartName }): JSX.Element {
  return (
    <>
      <Show when={props.menu}>
        {(m) => (
          <Menu trigger={{ icon: props.ios ? 'dots' : 'dots-v', 'aria-label': m().label, variant: 'ghost' }} items={m().items} onSelect={m().onSelect} placement="bottom-end" part={sub(props.part, 'menu')} />
        )}
      </Show>
      <Show when={props.confirm}>
        {(c) => (
          <span data-kit="app-bar-confirm">
            <Button variant="ghost" disabled={c().disabled === true} onClick={() => c().onSelect()} part={sub(props.part, 'confirm')}>
              {c().label}
            </Button>
          </span>
        )}
      </Show>
    </>
  );
}

function PhoneBar(props: TopAppBarProps & { readonly part: PartName }): JSX.Element {
  const t = useT(() => props.t);
  const look = usePlatform();
  const kind = (): 'root' | 'page' | 'compose' => props.kind ?? 'page';
  const ios = (): boolean => look() === 'ios';
  // M3 medium on a pushed page; the iOS large title on a destination's root; a compose bar is one line on both
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
        <Show when={props.back}>{(b) => <BackControl back={b()} ios={ios()} part={sub(props.part, 'back')} t={t} />}</Show>
        <Title title={props.title} onTitle={props.onTitle} menu={ios() ? props.titleMenu : undefined} part={sub(props.part, 'title')} />
        <div data-kit="app-bar-actions">
          <Show when={props.search}>
            {(q) => (
              <span data-kit="app-bar-search-icon">
                <IconButton icon="search" label={q().label} onClick={() => q().onOpen()} part={sub(props.part, 'search')} />
              </span>
            )}
          </Show>
          <Show when={props.actions?.length}>
            <Toolbar aria-label={props.actionsLabel ?? props.title} items={props.actions ?? []} justify="end" part={sub(props.part, 'actions')} t={t} />
          </Show>
          <PageEnd menu={props.menu} confirm={props.confirm} ios={ios()} part={props.part} />
          <ComposeButton when={ios() && kind() === 'root'} />
        </div>
      </div>
      <Show when={large()}>
        <div ref={largeEl} data-kit="app-bar-large">
          <Show
            when={ios() ? props.titleMenu : undefined}
            fallback={
              <Show when={props.onTitle} fallback={<span data-kit="app-bar-large-title" aria-hidden="true">{props.title}</span>}>
                {(go) => (
                  <button type="button" data-kit="app-bar-title-button" data-large="" aria-hidden="true" tabIndex={-1} onClick={() => go()()}>
                    <span data-kit="app-bar-large-title">{props.title}</span>
                    <Icon name="chevron" size="md" />
                  </button>
                )}
              </Show>
            }
          >
            {/* the large title opens the same pull-down, from under itself (the inline copy is hidden until it folds) */}
            {(m) => (
              <TitleMenu menu={m()} part={sub(sub(props.part, 'title'), 'large-menu')}>
                {(trigger) => (
                  <button {...trigger()} data-kit="app-bar-title-button" data-large="" aria-hidden="true" tabIndex={-1}>
                    <span data-kit="app-bar-large-title">{props.title}</span>
                    <Icon name="chevron" size="md" />
                  </button>
                )}
              </TitleMenu>
            )}
          </Show>
        </div>
      </Show>
      {/* iOS: the search field under the large title, folding away with it (the bar's icon takes over then) */}
      <Show when={ios() && kind() === 'root' ? props.search : undefined}>
        {(q) => (
          <div data-kit="app-bar-search">
            <button type="button" data-kit="app-bar-search-field" data-part={sub(props.part, 'search-field')} onClick={() => q().onOpen()}>
              <Icon name="search" size="sm" />
              <span data-text="control">{q().label}</span>
            </button>
          </div>
        )}
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
