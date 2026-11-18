// Menu and ContextMenu's machine half (Zag's menu, floating-ui): loaded apart from the first page (./lazy-part.ts), bound
// to the trigger Menu.tsx draws at once. Arrow keys, typeahead, Home/End, Escape back to the trigger.
// A submenu, on a desktop pointer (the browser with a hovering fine pointer), is a flyout beside its row (owner
// 2026-11-17): Zag's nested menu, a child machine per submenu, so hover intent, the safe triangle, flipping at the
// screen's edge and → / ← come from Zag. On touch and in the phone apps it drills in (v1 #786 ui/submenu.js): its items
// take the menu's place under a Back row, at the same spot and width; → opens it, ← or Escape steps back.
import * as menu from '@zag-js/menu';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, createSignal, createUniqueId, For, Match, onMount, Show, Switch, type Accessor, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { flag, sub, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { MenuBaseProps, MenuPlace } from './Menu.tsx';
import { bound, type Replay } from './lazy-part.ts';
import { tidySeparators, type MenuEntry, type MenuGroup, type MenuItem, type MenuSeparator, type MenuSubmenu } from './menu-entries.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { fromPoint, underBox } from './menu-anchor.ts';
import { Flyout, flyoutPointer, type FlyContext } from './menu-flyout.tsx';

const BACK = 'menu:back';

interface MenuState {
  readonly api: Accessor<menu.Api>;
  /** the submenu drilled into (its value), or none */
  readonly pane: Accessor<string | undefined>;
  readonly drill: (value: string | undefined) => void;
  /** the menu's width as it drilled in: it keeps it in and out of the submenu (v1) */
  readonly width: Accessor<string | undefined>;
  readonly setBox: (el: HTMLElement) => void;
  /** back to the top level at its own width (a right-click on another row re-anchors the open menu) */
  readonly reset: () => void;
  /** submenus fly out (desktop pointer) rather than drill in; read again each time the menu opens */
  readonly fly: Accessor<FlyContext | undefined>;
}

const firstItem = (items: readonly MenuEntry[]): string | undefined => {
  for (const e of items) {
    if (e.kind === 'separator') continue;
    if (e.kind === 'group') {
      const x = e.items.find((i) => !i.disabled);
      if (x) return x.value;
    } else if (!(e.kind !== 'submenu' && e.disabled)) return e.value;
  }
  return undefined;
};

function flyContext(parent: menu.Service, onSelect: (value: string) => void, kids: Set<() => void>): FlyContext {
  return {
    parent,
    onSelect,
    adopt: (close) => {
      kids.add(close);
      return () => kids.delete(close);
    },
    Entries,
  };
}

/** where the menu goes: at the pointer (a right-click: v1 clampToViewport, opened beside it and slid up or in to fit,
 * never flipped), else under its trigger; a phone's is drawn over the screen (fixed): as it grows drilling into a
 * submenu it never widens the page (QA r7 #4: WebKit kept the frame where the wider menu stood before it moved, and the
 * page scrolled sideways 11 px) */
/** what the menu is placed against: a point while `from` gives one (else the trigger, or the box `below` names) */
function anchorOf(props: MenuPlace) {
  const from = props.from;
  if (from) return { getAnchorRect: fromPoint(from, props.below !== undefined ? underBox(props.below) : undefined) };
  return props.below !== undefined ? { getAnchorRect: underBox(props.below) } : {};
}

function positioningOf(at: 'pointer' | undefined, props: MenuPlace, look: string) {
  const base = at === 'pointer' ? { placement: 'bottom-start' as const, gutter: 0, flip: false, overlap: true, overflowPadding: 8 } : { placement: props.placement ?? 'bottom-start', gutter: 4, overlap: true, overflowPadding: 8, ...anchorOf(props) };
  return { ...base, ...(look !== 'web' && { strategy: 'fixed' as const }) };
}

function useMenu(props: MenuBaseProps & MenuPlace, at?: 'pointer'): MenuState {
  const id = createUniqueId();
  const [chosen, setPane] = createSignal<string | undefined>();
  const [width, setWidth] = createSignal<string | undefined>();
  // the submenu drilled into, while the items still have it (a task's More can drop a row and fold it inline)
  const pane = (): string | undefined => {
    const v = chosen();
    return v !== undefined && props.items.some((e) => e.kind === 'submenu' && e.value === v) ? v : undefined;
  };
  const look = usePlatform();
  const [flying, setFlying] = createSignal(flyoutPointer(look()));
  // the open flyouts' closes: the menu closing or re-anchoring closes them at once
  const kids = new Set<() => void>();
  const reset = (): void => {
    setPane(undefined);
    setWidth(undefined);
    for (const close of kids) close();
  };
  let box: HTMLElement | undefined;
  const subs = createMemo(() => new Set(props.items.flatMap((e) => (e.kind === 'submenu' ? [e.value] : []))));
  let api: Accessor<menu.Api> = () => {
    throw new Error('menu: not connected');
  };
  // in or out of a submenu: the first of its items lit (or, coming back, the row that opened it)
  const drill = (value: string | undefined): void => {
    const was = pane();
    if (value !== undefined && was === undefined && box) setWidth(`${box.offsetWidth}px`);
    setPane(value);
    const sub = props.items.find((e): e is MenuSubmenu => e.kind === 'submenu' && e.value === value);
    const lit = sub ? firstItem(sub.items) : was;
    if (lit) queueMicrotask(() => api().setHighlightedValue(lit));
  };
  const service = useMachine(menu.machine, () => ({
    id,
    onSelect: (d: menu.SelectionDetails) => {
      if (d.value === BACK) drill(undefined);
      else if (subs().has(d.value)) drill(d.value);
      else props.onSelect(d.value);
    },
    // Escape inside a submenu steps back to the menu rather than closing it (v1 wireSubmenus)
    onEscapeKeyDown: (e: KeyboardEvent) => {
      if (pane() === undefined) return;
      e.preventDefault();
      drill(undefined);
    },
    ...(props.open === undefined ? {} : { open: props.open }),
    onOpenChange: (d: menu.OpenChangeDetails) => {
      if (!d.open) reset();
      else setFlying(flyoutPointer(look()));
      props.onOpenChange?.(d.open);
    },
    positioning: positioningOf(at, props, look()),
  }));
  api = createMemo(() => menu.connect(service, normalizeProps));
  const ctx = flyContext(service, (v) => props.onSelect(v), kids);
  return { api, pane, drill, width, setBox: (el) => (box = el), reset, fly: () => (flying() ? ctx : undefined) };
}

function Item(props: { readonly api: menu.Api; readonly item: MenuItem; readonly part: PartName; readonly iconCol?: boolean }): JSX.Element {
  return (
    <div
      {...props.api.getItemProps({ value: props.item.value, disabled: props.item.disabled, valueText: props.item.label })}
      // a toggle (Transcript while the steps show) says it's on, as its check does (QA round 6 desktop #6; the phone's
      // menu has done so since step 13)
      role={props.item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
      aria-checked={props.item.checked === undefined ? undefined : props.item.checked}
      data-kit="menu-item"
      data-part={props.item.part ?? sub(props.part, 'item')}
      data-danger={flag(props.item.danger)}
      data-value={props.item.value}
    >
      {/* no icon of its own in a menu where others have one: the column stays, so its words line up with theirs */}
      <Show when={props.item.icon} fallback={props.iconCol ? <span data-kit="menu-icon-gap" aria-hidden="true" /> : undefined}>
        {(name) => <Icon name={name()} size="md" />}
      </Show>
      <span data-kit="menu-item-text">
        <span data-kit="menu-item-label" data-text="control">
          {props.item.label}
        </span>
        <Show when={props.item.hint}>{(hint) => <span data-kit="menu-item-hint">{hint()}</span>}</Show>
      </span>
      <Show when={props.item.checked}>
        <Icon name="check" size="sm" tone="accent-ink" />
      </Show>
      <Show when={props.item.keys}>{(keys) => <kbd data-kit="kbd">{keys()}</kbd>}</Show>
    </div>
  );
}

/** A submenu's row: its icon, its name and a chevron; choosing it (or →) drills in. */
function SubRow(props: { readonly api: menu.Api; readonly entry: MenuSubmenu; readonly part: PartName }): JSX.Element {
  return (
    <div
      {...props.api.getItemProps({ value: props.entry.value, closeOnSelect: false, valueText: props.entry.label })}
      data-kit="menu-item"
      data-part={props.entry.part ?? sub(props.part, 'sub')}
      data-value={props.entry.value}
      data-sub=""
      aria-haspopup="menu"
      aria-expanded="false"
    >
      <Show when={props.entry.icon}>{(name) => <Icon name={name()} size="md" />}</Show>
      <span data-kit="menu-item-text">
        <span data-kit="menu-item-label" data-text="control">
          {props.entry.label}
        </span>
      </span>
      <Icon name="chevron" size="sm" tone="faint" />
    </div>
  );
}

/** A submenu's Back row: names the submenu it's in. */
function BackRow(props: { readonly api: menu.Api; readonly label: string; readonly part: PartName }): JSX.Element {
  return (
    <div {...props.api.getItemProps({ value: BACK, closeOnSelect: false, valueText: props.label })} data-kit="menu-item" data-part={sub(props.part, 'back')} data-back="">
      <Icon name="back" size="md" />
      <span data-kit="menu-item-text">
        <span data-kit="menu-item-label" data-text="control">
          {props.label}
        </span>
      </span>
    </div>
  );
}

function Entries(props: { readonly api: menu.Api; readonly items: readonly MenuEntry[]; readonly part: PartName; readonly fly?: FlyContext | undefined }): JSX.Element {
  // any icon in this pane: then every row keeps the icon column (QA r1 #29: Lean runs' choices sat left of the rest)
  const iconCol = createMemo(() => props.items.some((e) => (e.kind === 'group' ? e.items.some((i) => i.icon) : e.kind !== 'separator' && !!e.icon)));
  return (
    <For each={tidySeparators(props.items)}>
      {(entry) => (
        <Switch fallback={<Item api={props.api} item={entry as MenuItem} part={props.part} iconCol={iconCol()} />}>
          <Match when={entry.kind === 'separator'}>
            <div {...props.api.getSeparatorProps()} data-kit="menu-separator" data-part={sub(props.part, 'separator')} />
          </Match>
          <Match when={entry.kind === 'submenu' && entry}>
            {(e) => (
              <Show when={props.fly} fallback={<SubRow api={props.api} entry={e()} part={props.part} />}>
                {(fly) => <Flyout api={props.api} entry={e()} part={props.part} fly={fly()} />}
              </Show>
            )}
          </Match>
          <Match when={entry.kind === 'group' && entry}>
            {(group) => (
              <div {...props.api.getItemGroupProps({ id: group().id })} data-kit="menu-group" data-part={sub(props.part, 'group')}>
                <div {...props.api.getItemGroupLabelProps({ htmlFor: group().id })} data-kit="menu-group-label">
                  {group().label}
                </div>
                <For each={group().items}>{(item) => <Item api={props.api} item={item} part={props.part} iconCol={iconCol()} />}</For>
              </div>
            )}
          </Match>
        </Switch>
      )}
    </For>
  );
}

/** The floating list both menus share. */
function MenuContent(props: { readonly state: MenuState; readonly items: readonly MenuEntry[]; readonly part: PartName; readonly at?: 'pointer' }): JSX.Element {
  const api = (): menu.Api => props.state.api();
  const open = createMemo(() => {
    const v = props.state.pane();
    return v === undefined ? undefined : props.items.find((e): e is MenuSubmenu => e.kind === 'submenu' && e.value === v);
  });
  const drill = (v: string | undefined): void => props.state.drill(v);
  // → on a submenu's row opens it, ← in one steps back (before Zag's own keys)
  const keys = (e: KeyboardEvent): void => {
    if (props.state.fly()) return;
    const lit = api().highlightedValue;
    if (e.key === 'ArrowRight' && !open() && lit && props.items.some((x) => x.kind === 'submenu' && x.value === lit)) drill(lit);
    else if (e.key === 'ArrowLeft' && open()) drill(undefined);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  return (
    <Portal>
      <div {...api().getPositionerProps()} data-kit="positioner" ref={(el) => el.addEventListener('keydown', keys, true)}>
        <div
          {...api().getContentProps()}
          ref={(el) => props.state.setBox(el)}
          data-kit="menu"
          data-part={props.part}
          data-at={props.at}
          data-drilled={flag(!!open())}
          style={props.state.width() ? { '--menu-w': props.state.width() } : undefined}
        >
          {/* rows only while open: a closed menu (the open task's More, rebuilt by every patch to the task) holds
              none, and no flyout portals (perf audit 2026-11-18 #12) */}
          <Show when={api().open}>
            <Show when={open()} fallback={<Entries api={api()} items={props.items} part={props.part} fly={props.state.fly()} />}>
              {(s) => (
                <div role="group" aria-label={s().label} data-kit="menu-pane" data-part={sub(props.part, 'pane')}>
                  <BackRow api={api()} label={s().label} part={props.part} />
                  <Entries api={api()} items={s().items} part={props.part} />
                </div>
              )}
            </Show>
          </Show>
        </div>
      </div>
    </Portal>
  );
}


/** What Menu.tsx hands the machine: the menu's props, how it opens, and where to report the trigger's props. */
export interface MenuMachineProps {
  readonly base: MenuBaseProps & MenuPlace;
  readonly at?: 'pointer';
  readonly part: PartName;
  /** the trigger's props from now on (the button's, or the right-click area's) */
  readonly bind: (props: Accessor<Record<string, unknown>>) => void;
  /** pressed before the machine was there: that press again, now */
  readonly pending?: Replay | undefined;
  /** the right-click area */
  readonly area?: () => HTMLElement | undefined;
}

export function MenuMachine(props: MenuMachineProps): JSX.Element {
  const state = useMenu(props.base, props.at);
  props.bind(() => (props.at === 'pointer' ? state.api().getContextTriggerProps() : state.api().getTriggerProps()) as Record<string, unknown>);
  onMount(() => {
    // a right-click on another row re-anchors the open menu at its top level
    props.area?.()?.addEventListener('contextmenu', state.reset, true);
    bound(props.at === 'pointer' ? undefined : state.api().getTriggerProps().id, props.pending);
  });
  return <MenuContent state={state} items={props.base.items} part={props.part} {...(props.at ? { at: props.at } : {})} />;
}
