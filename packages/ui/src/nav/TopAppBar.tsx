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
// `selection` is iOS's edit mode on a root bar (Mail, Files): the bar stays where it is, its large title (or the folded
// title, scrolled) says how many are picked, Select All leads and Done ends; the screen's actions, title menu and
// search step aside, so the list under it never moves (AGENTS.md "Nothing jumps"). On Android and in the browser the
// bar gives way while `selection` is set: the screen's SelectionBar (M3's contextual bar, at the same height) takes its
// place. So a screen renders both and the kit picks, features never ask the platform.
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
import { selectionWords } from './selection-words.ts';

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

/** iOS's edit mode on a root bar (ADR-0065 step 13): what's picked and the bar's two buttons. */
export interface TopAppBarSelection {
  /** how many rows are picked */
  readonly count: number;
  /** every row the list shows is picked: Select All turns into Deselect All */
  readonly allPicked: boolean;
  readonly onAll: () => void;
  /** Done */
  readonly onExit: () => void;
  /** its parts' base (default: the bar's part + `.select`): `.all`, `.title`, `.exit` */
  readonly part?: PartName;
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
  /** The screen's one selector (the Tasks list's project pick): on Android (M3) the top app bar's title menu, its
   *  `title` with the menu arrow, opening `onOpen`, and the action `action` leaves the actions; iOS and the browser keep
   *  `title` and the action as given. Ignored while `onTitle` is set. Additive (QA round 3 A10, 2026-11-18). */
  readonly titleSelect?: { readonly title: string; readonly onOpen: () => void; readonly action: string };
  readonly actions?: readonly ToolbarItem[];
  /** The actions toolbar's name (default: the title). */
  readonly actionsLabel?: string;
  readonly t?: Translate;
  /** A task page (the phone apps): swiping the head left / right opens the next / previous task in the list's order
   *  (design/mobile.md §A.4); the twins are Next task / Previous task in `actions`' menu. The browser binds nothing. */
  readonly swipe?: TaskSwipe;
  /** The list under this bar picks rows. iOS: the bar in edit mode (any kind; a list's root bar in practice) (Select All, the count, Done), keeping its
   *  place and height. Android and the browser: the bar draws nothing, the screen's SelectionBar (the contextual bar)
   *  stands in its place. Additive (2026-11-15). */
  readonly selection?: TopAppBarSelection;
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
  // Android: the screen's selector is the title menu (M3), not one more control among the actions
  const select = (): TopAppBarProps['titleSelect'] => (!ios() && !props.onTitle ? props.titleSelect : undefined);
  const title = (): string => select()?.title ?? props.title;
  const onTitle = (): (() => void) | undefined => select()?.onOpen ?? props.onTitle;
  const actions = (): readonly ToolbarItem[] => {
    const s = select();
    return s ? (props.actions ?? []).filter((a) => a.id !== s.action) : (props.actions ?? []);
  };
  // M3 medium on a pushed page; the iOS large title on a destination's root; a compose bar is one line on both
  const large = (): boolean => (ios() ? kind() === 'root' : kind() === 'page');
  const host = useComposeHost();
  claimCompose(host, ios() && kind() === 'root');
  const [collapsed, setCollapsed] = createSignal(false);
  const swipe = useTaskSwipe(() => props.swipe ?? {});
  // iOS edit mode: the same header (its large title, its folded state), its buttons and words swapped
  // (any kind: TopAppBar keeps the bar for `selection` on iOS whatever its kind, so it must offer Done)
  const editing = (): TopAppBarSelection | undefined => (ios() ? props.selection : undefined);
  const selPart = (): PartName => editing()?.part ?? sub(props.part, 'select');
  const words = () => {
    const e = editing();
    return e ? selectionWords(t, true, e.count, e.allPicked) : undefined;
  };
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
      data-kit="app-bar" data-part={props.part} data-kind={kind()} data-large={flag(large())} data-collapsed={flag(large() && collapsed())} data-selecting={flag(editing() !== undefined)}>
      <Show when={editing()} fallback={
      <div data-kit="app-bar-row">
        <Show when={props.back}>{(b) => <BackControl back={b()} ios={ios()} part={sub(props.part, 'back')} t={t} />}</Show>
        <Title title={title()} onTitle={onTitle()} menu={ios() ? props.titleMenu : undefined} part={sub(props.part, 'title')} />
        <div data-kit="app-bar-actions">
          <Show when={props.search}>
            {(q) => (
              <span data-kit="app-bar-search-icon">
                <IconButton icon="search" label={q().label} onClick={() => q().onOpen()} part={sub(props.part, 'search')} />
              </span>
            )}
          </Show>
          <Show when={actions().length}>
            <Toolbar aria-label={props.actionsLabel ?? props.title} items={actions()} justify="end" part={sub(props.part, 'actions')} t={t} />
          </Show>
          <PageEnd menu={props.menu} confirm={props.confirm} ios={ios()} part={props.part} />
          <ComposeButton when={ios() && kind() === 'root'} />
        </div>
      </div>
      }>
        {(e) => (
          <div data-kit="app-bar-row">
            <Button variant="ghost" onClick={() => e().onAll()} part={sub(selPart(), 'all')}>
              {words()?.all}
            </Button>
            <h1 data-kit="app-bar-title" data-text="control" data-part={sub(selPart(), 'title')} aria-live="polite">
              {words()?.title}
            </h1>
            <div data-kit="app-bar-actions">
              <span data-kit="app-bar-confirm">
                <Button variant="ghost" onClick={() => e().onExit()} part={sub(selPart(), 'exit')}>
                  {t('done')}
                </Button>
              </span>
            </div>
          </div>
        )}
      </Show>
      <Show when={large()}>
        <div ref={largeEl} data-kit="app-bar-large">
          <Show when={words()} fallback={
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
          }>
            {(w) => <span data-kit="app-bar-large-title" aria-hidden="true">{w().title}</span>}
          </Show>
        </div>
      </Show>
      {/* iOS: the search field under the large title, folding away with it (the bar's icon takes over then) */}
      <Show when={ios() && kind() === 'root' ? props.search : undefined}>
        {(q) => (
          <div data-kit="app-bar-search">
            <button type="button" data-kit="app-bar-search-field" data-part={sub(props.part, 'search-field')} disabled={editing() !== undefined} onClick={() => q().onOpen()}>
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
  // selection: iOS keeps this bar (edit mode, nothing remounts); elsewhere SelectionBar takes its place
  const shown = (): boolean => look() === 'ios' || props.selection === undefined;
  return (
    <Show when={shown()}>
      <Show when={look() !== 'web'} fallback={<WebBar {...props} part={part()} />}>
        <PhoneBar {...props} part={part()} />
      </Show>
    </Show>
  );
}
