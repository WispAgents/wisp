// My Stuff in the phone apps (ADR-0065 step 14, design/mobile.md §A.2 My Stuff): the Stuff destination's root screen.
// The platform's root bar (M3 small top app bar; the iOS large title) titled My Stuff, with Search, Upload and the
// screen's ⋮ / ⋯ (Make a pane, by time / by task, grid / list); the kinds under it (Android: M3 filter chips; iOS: a
// segmented control); then what's kept, grouped as the browser groups it, as a grid of tiles (two columns: the
// thumbnail or the kind's glyph, the name and its facts under it, Files-like) or as a list of the platform's rows. Each
// thing opens on a tap; its ⋯ holds every action (v1's item menu) and is on screen, so a list row's swipes (Share to
// the right, Delete to the left: `stuff.swipe`) are never the only way. A launched app opens over the whole screen in
// its sandboxed frame with the platform's bar and Close (StuffPhoneApp, F-stuff-10). The words and the actions come
// from the caller (wisp.stuff); this draws them. Imported by the stuff plugin's phone chunk only.
import { For, Show, createMemo, createSignal, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import type { Translate } from '../core/i18n.tsx';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Button, IconButton } from '../controls/Button.tsx';
import { Dialog } from '../controls/Dialog.tsx';
import { Input } from '../controls/Input.tsx';
import { Menu, type MenuEntry } from '../controls/Menu.tsx';
import { Tabs } from '../controls/Tabs.tsx';
import { EmptyState } from '../composites/EmptyState.tsx';
import { ListRow } from '../composites/ListRow.tsx';
import { StuffThumbBody, type StuffThumb } from './StuffCard.tsx';
import type { ToolbarItem } from '../composites/Toolbar.tsx';
import { AppFrame } from '../frame/AppFrame.tsx';
import { TopAppBar, type TopAppBarMenu } from '../nav/TopAppBar.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { Swipe, type SwipeDir, type SwipeSide } from '../shell-list/Swipe.tsx';
import '../css/phone-pages.css';

export interface StuffPhoneItem {
  readonly id: string;
  readonly title: string;
  /** its facts (kind · size · who kept it) */
  readonly sub: string;
  /** when it was kept, in words */
  readonly when: string;
  readonly icon: IconName;
  /** a picture's own image, or an app's cover: shown in its thumbnail in place of the glyph (QA r2 phone #13) */
  readonly image?: string;
  /** a doc's page, code's lines, an app's window: the card's preview, in its tile (QA phone r3 #5; a row keeps its glyph) */
  readonly thumb?: StuffThumb;
  /** its ⋯ menu: every action it takes (open, share, star, the task it came from, delete, …) */
  readonly menu: readonly MenuEntry[];
}

export interface StuffPhoneGroup {
  readonly id: string;
  readonly label: string;
  readonly items: readonly StuffPhoneItem[];
}

export interface StuffPhoneKind {
  readonly id: string;
  readonly label: string;
  readonly count: number;
}

export interface StuffPhoneProps extends KitProps {
  readonly title: string;
  /** Add files (design/mobile.md: Upload): Android's bar icon after Search, iOS's first entry in the ⋯ menu */
  readonly add: { readonly label: string; readonly onSelect: () => void };
  readonly actionsLabel: string;
  /** the screen's ⋮ / ⋯ */
  readonly menu: TopAppBarMenu;
  readonly search: { readonly value: string; readonly label: string; readonly placeholder: string; readonly onInput: (v: string) => void };
  /** the kinds there are (more than one: else none is drawn) */
  readonly kinds: { readonly label: string; readonly items: readonly StuffPhoneKind[]; readonly value: string; readonly onPick: (id: string) => void };
  readonly view: 'grid' | 'list';
  readonly groups: readonly StuffPhoneGroup[];
  readonly onOpen: (id: string) => void;
  readonly onMenu: (id: string, action: string) => void;
  /** the ⋯ button's name for a thing */
  readonly menuLabel: (title: string) => string;
  /** a list row's swipes; their actions are in each row's ⋯ too */
  readonly swipe: { readonly right: SwipeSide; readonly left: SwipeSide; readonly onSwipe: (id: string, dir: SwipeDir) => void };
  /** nothing kept yet */
  readonly empty?: { readonly title: string; readonly body: string; readonly action: string; readonly onAction: () => void } | undefined;
  /** kept things, none matching the search or kind */
  readonly nothing: string;
  readonly t?: Translate;
}

const same = (a: readonly string[], b: readonly string[]): boolean => a.length === b.length && a.every((x, i) => x === b[i]);

/** Android's filter chips or iOS's segmented control, for the same choices. */
function Kinds(props: { readonly kinds: StuffPhoneProps['kinds']; readonly part: PartName }): JSX.Element {
  const look = usePlatform();
  return (
    <div data-kit="stuff-phone-kinds" data-part={props.part} role={look() === 'ios' ? undefined : 'group'} aria-label={look() === 'ios' ? undefined : props.kinds.label}>
      <Show
        when={look() === 'ios'}
        fallback={
          <For each={props.kinds.items}>
            {(k) => (
              <Button variant="chip" size="sm" pressed={props.kinds.value === k.id} {...(props.kinds.value === k.id && { icon: 'check' as const })} onClick={() => props.kinds.onPick(k.id)}>
                {`${k.label} ${k.count}`}
              </Button>
            )}
          </For>
        }
      >
        <Tabs
          variant="pill"
          fill
          aria-label={props.kinds.label}
          items={props.kinds.items.map((k) => ({ value: k.id, label: k.label, count: k.count }))}
          value={props.kinds.value}
          onChange={(v) => props.kinds.onPick(v)}
        />
      </Show>
    </div>
  );
}

function ItemMenu(props: { readonly item: StuffPhoneItem; readonly label: string; readonly part: PartName; readonly onSelect: (v: string) => void }): JSX.Element {
  return <Menu part={props.part} trigger={{ icon: 'dots', 'aria-label': props.label, variant: 'ghost', size: 'sm' }} items={props.item.menu} onSelect={props.onSelect} placement="bottom-end" />;
}

/** the ⋯ menu's Add files on iOS */
const ADD = 'stuff-phone.add';

export function StuffPhone(props: StuffPhoneProps): JSX.Element {
  const part = (): PartName => props.part ?? 'stuff.phone';
  const look = usePlatform();
  // icons only in an app bar (M3); iOS puts Upload in the bar's ⋯ menu (HIG: one or two bar buttons)
  const actions = (): ToolbarItem[] =>
    look() === 'ios'
      ? []
      : [{ id: 'add', label: props.add.label, icon: 'plus', priority: 'pin', render: () => <IconButton icon="plus" label={props.add.label} part={sub(part(), 'add')} onClick={() => props.add.onSelect()} /> }];
  const menu = (): TopAppBarMenu =>
    look() === 'ios'
      ? {
          label: props.menu.label,
          items: [{ value: ADD, label: props.add.label, icon: 'plus' }, { kind: 'separator' }, ...props.menu.items],
          onSelect: (v) => (v === ADD ? props.add.onSelect() : props.menu.onSelect(v)),
        }
      : props.menu;
  const [searching, setSearching] = createSignal(false);
  const field = (): boolean => searching() || props.search.value !== '';
  const count = (): number => props.groups.reduce((n, g) => n + g.items.length, 0);
  // keyed by id, not by the objects the caller rebuilds on every change: a tile or a row stays the same node (its open
  // menu, its swipe) while what it shows follows (AGENTS.md "Nothing jumps")
  const byId = createMemo(() => new Map(props.groups.flatMap((g) => g.items.map((i) => [i.id, i] as const))));
  const groupOf = createMemo(() => new Map(props.groups.map((g) => [g.id, g] as const)));
  const groupIds = createMemo(() => props.groups.map((g) => g.id), undefined, { equals: same });
  const menuOf = (it: () => StuffPhoneItem): JSX.Element => (
    <ItemMenu item={it()} label={props.menuLabel(it().title)} part={sub(part(), 'row.menu')} onSelect={(v) => props.onMenu(it().id, v)} />
  );
  const Tile = (p: { readonly id: string }): JSX.Element => (
    <Show when={byId().get(p.id)}>
      {(it) => (
        <li data-kit="stuff-tile" data-part={sub(part(), 'tile')} data-id={p.id}>
          <button type="button" data-kit="stuff-tile-open" data-part={sub(part(), 'tile.open')} onClick={() => props.onOpen(p.id)}>
            <span data-kit="stuff-tile-thumb" data-preview={it().thumb ? it().thumb?.kind : undefined} aria-hidden="true">
              <Show
                when={it().image}
                fallback={
                  <Show when={it().thumb} fallback={<Icon name={it().icon} size="lg" />}>
                    {(th) => <StuffThumbBody thumb={th()} />}
                  </Show>
                }
              >
                {(src) => <img src={src()} alt="" loading="lazy" decoding="async" />}
              </Show>
            </span>
            <span data-kit="stuff-tile-name">
              <span data-text="control">{it().title}</span>
              <span data-text="control">{it().when ? `${it().when} · ${it().sub}` : it().sub}</span>
            </span>
          </button>
          <span data-kit="stuff-tile-menu">{menuOf(it)}</span>
        </li>
      )}
    </Show>
  );
  const Row = (p: { readonly id: string }): JSX.Element => (
    <Show when={byId().get(p.id)}>
      {(it) => (
        <Swipe gesture="stuff.swipe" part={sub(part(), 'swipe')} right={props.swipe.right} left={props.swipe.left} onSwipe={(d) => props.swipe.onSwipe(p.id, d)}>
          <ListRow
            part={sub(part(), 'row')}
            title={it().title}
            sub={it().sub}
            lead={
              <span data-kit="stuff-thumb" aria-hidden="true">
                <Show when={it().image} fallback={<Icon name={it().icon} size="md" />}>
                  {(src) => <img src={src()} alt="" loading="lazy" decoding="async" />}
                </Show>
              </span>
            }
            time={it().when}
            actions={menuOf(it)}
            onOpen={() => props.onOpen(p.id)}
          />
        </Swipe>
      )}
    </Show>
  );
  const Group = (p: { readonly id: string }): JSX.Element => {
    const g = (): StuffPhoneGroup | undefined => groupOf().get(p.id);
    const ids = createMemo(() => g()?.items.map((i) => i.id) ?? [], undefined, { equals: same });
    return (
      <section data-kit="stuff-phone-group" data-part={sub(part(), 'group')} aria-label={g()?.label}>
        <h2 data-kit="stuff-phone-group-head" data-part={sub(part(), 'group.head')}>
          {g()?.label}
        </h2>
        <Show
          when={props.view === 'list'}
          fallback={
            <ul data-kit="stuff-phone-grid">
              <For each={ids()}>{(id) => <Tile id={id} />}</For>
            </ul>
          }
        >
          <div data-kit="stuff-phone-rows" role="list">
            <For each={ids()}>{(id) => <Row id={id} />}</For>
          </div>
        </Show>
      </section>
    );
  };
  return (
    <div data-kit="phone-page" data-part={part()} data-view={props.view}>
      <TopAppBar
        kind="root"
        part={sub(part(), 'bar')}
        title={props.title}
        search={{ label: props.search.label, onOpen: () => setSearching(true) }}
        actions={actions()}
        actionsLabel={props.actionsLabel}
        menu={menu()}
        {...(props.t && { t: props.t })}
      />
      <div data-kit="phone-page-body" data-part={sub(part(), 'body')}>
        <Show when={field()}>
          <div data-kit="stuff-phone-search" data-part={sub(part(), 'search')}>
            <Input
              type="search"
              value={props.search.value}
              placeholder={props.search.placeholder}
              aria-label={props.search.label}
              autofocus
              onInput={props.search.onInput}
              onBlur={() => props.search.value === '' && setSearching(false)}
              onKeyDown={(e: KeyboardEvent) => {
                if (e.key !== 'Escape') return;
                e.preventDefault();
                props.search.onInput('');
                setSearching(false);
              }}
            />
          </div>
        </Show>
        <Show when={props.kinds.items.length > 1}>
          <Kinds kinds={props.kinds} part={sub(part(), 'kinds')} />
        </Show>
        <Show
          when={!props.empty}
          fallback={
            <Show when={props.empty}>
              {(e) => (
                <div data-kit="stuff-phone-empty">
                  <EmptyState
                    icon="stuff"
                    title={e().title}
                    action={
                      <Button variant="primary" icon="plus" part={sub(part(), 'empty.add')} onClick={() => e().onAction()}>
                        {e().action}
                      </Button>
                    }
                  >
                    {e().body}
                  </EmptyState>
                </div>
              )}
            </Show>
          }
        >
          <Show when={count() > 0} fallback={<EmptyState compact icon="search" title={props.nothing} />}>
            <For each={groupIds()}>{(id) => <Group id={id} />}</For>
          </Show>
        </Show>
      </div>
    </div>
  );
}

export interface StuffPhoneAppProps extends KitProps {
  readonly title: string;
  readonly src: string;
  readonly sandbox: string;
  readonly onClose: () => void;
  /** Android's × (Close) */
  readonly closeLabel: string;
  /** iOS's word for it (Done) */
  readonly doneLabel: string;
}

/** A launched app over the whole screen (design/mobile.md §A.7 StuffFrame): the platform's bar with Close / Done, then
 *  the app in its sandboxed frame. */
export function StuffPhoneApp(props: StuffPhoneAppProps): JSX.Element {
  const part = (): PartName => props.part ?? 'stuff.phone.app';
  return (
    <Dialog open bare size="full" title={props.title} onOpenChange={(open) => !open && props.onClose()} part={part()}>
      <div data-kit="phone-page" data-part={sub(part(), 'page')}>
        <TopAppBar kind="compose" part={sub(part(), 'bar')} title={props.title} back={{ label: props.closeLabel, onBack: () => props.onClose(), close: props.doneLabel }} />
        <div data-kit="stuff-phone-app">
          <AppFrame title={props.title} src={props.src} sandbox={props.sandbox} />
        </div>
      </div>
    </Dialog>
  );
}
