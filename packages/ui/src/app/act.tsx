// What pressing one of the frame's controls does (design/feedback.md): a press, or a menu it opens
// (the kit's menu, on the control as v1 draws it). While its command is in flight the control is
// pending: a spinner in place of its icon, the same width, and pressing it again does nothing.
import { Show, createEffect, createMemo, createSignal, splitProps, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import { MenuOn, type MenuEntry, type Placement } from '../controls/Menu.tsx';
import type { MenuFilter } from '../controls/menu-entries.ts';
import { Spinner } from '../controls/Spinner.tsx';
import { lazyPart, warm } from '../controls/lazy-part.ts';
import type { PartName } from '../core/props.ts';

/** the panel's popover, apart from the first page (most of it is Zag's) */
const panelPart = lazyPart(() => import('./act-panel.tsx'));

export interface Act {
  readonly onClick?: () => void;
  readonly menu?: {
    readonly items: readonly MenuEntry[];
    readonly onSelect: (value: string) => void;
    /** the pointer or the focus reached its control: fetch what its rows need (a lazy module, its answers) before the
     *  press. Additive (2026-11-20). */
    readonly onWarm?: () => void;
    /** as a submenu (the composer's `+`), the box that narrows its rows (MenuSubmenu.filter). Additive (2026-11-20). */
    readonly filter?: MenuFilter;
    /** its sheet's head where it opens as one (SheetMenus: the phone composer's). Additive (2026-11-21). */
    readonly title?: string;
  };
  /** its command is in flight */
  readonly pending?: boolean;
  /** A panel it opens beside it (v1's Delegate .team-pop): what's in it, drawn while open. Additive (2026-11-17). */
  readonly panel?: {
    readonly label: string;
    readonly part: PartName;
    readonly cardPart?: PartName;
    readonly placement?: Placement;
    readonly content: () => JSX.Element;
    /** opened from outside its button (the composer's `+` menu: Delegate…): it follows `open` and says when it
     *  closes. Additive (2026-11-18). */
    readonly open?: boolean;
    readonly onOpenChange?: (open: boolean) => void;
  };
}

/** The attributes a control gets for its act: pending (aria-busy, data-pending) and no second press. */
export function actAttrs(act: Act | undefined): Record<string, unknown> {
  return act?.pending ? { 'aria-busy': 'true', 'data-pending': '' } : {};
}

/** Draws `render(trigger)`: with a menu, `trigger()` holds the menu's trigger props to spread on the
 * control; without one, its press. */
export function WithAct(props: { readonly act?: Act | undefined; readonly render: (trigger: () => Record<string, unknown>) => JSX.Element }): JSX.Element {
  // read once per change under this owner: the press reads it in its click handler, where reading the caller's prop
  // getter (a compiled memo for `act={a ? x : y}`) made a new unowned memo on every press (QA round 1 #15)
  const act = createMemo(() => props.act);
  const press = (): Record<string, unknown> => ({
    onClick: (e: MouseEvent) => {
      e.stopPropagation();
      const now = act();
      if (!now?.pending) now?.onClick?.();
    },
    ...actAttrs(act()),
  });
  return (
    <Show when={props.act?.panel} fallback={<WithMenu act={props.act} render={props.render} press={press} />}>
      {(p) => <WithPanel panel={p()} act={props.act} render={props.render} />}
    </Show>
  );
}

function WithPanel(props: { readonly panel: NonNullable<Act['panel']>; readonly act?: Act | undefined; readonly render: (trigger: () => Record<string, unknown>) => JSX.Element }): JSX.Element {
  const [own, setOwn] = createSignal(false);
  // its own while nobody else holds it; the caller's `open` when it does (and it hears every change)
  const open = (): boolean => props.panel.open ?? own();
  const setOpen = (o: boolean): void => {
    setOwn(o);
    props.panel.onOpenChange?.(o);
  };
  createEffect(() => {
    if (props.panel.open) panelPart.load().catch(() => setOpen(false));
  });
  // before its popover has loaded: the button as it looks, a press that opens it once it's here
  const early = (): Record<string, unknown> => ({
    'aria-haspopup': 'dialog',
    'aria-expanded': 'false',
    onPointerEnter: warm(panelPart),
    onFocus: warm(panelPart),
    onClick: (e: MouseEvent) => {
      e.stopPropagation();
      setOpen(true);
      panelPart.load().catch(() => setOpen(false));
    },
    ...actAttrs(props.act),
  });
  return (
    <Show when={panelPart.ready()} fallback={props.render(early)}>
      {(m) => (
        <ActPanelOf
          m={m()}
          label={props.panel.label}
          part={props.panel.part}
          cardPart={props.panel.cardPart}
          placement={props.panel.placement ?? 'bottom-start'}
          open={open()}
          onOpenChange={setOpen}
          content={props.panel.content}
          render={(trigger: () => Record<string, unknown>) => props.render(() => ({ ...trigger(), ...actAttrs(props.act) }))}
        />
      )}
    </Show>
  );
}

type PanelModule = typeof import('./act-panel.tsx');
function ActPanelOf(props: { readonly m: PanelModule } & Parameters<PanelModule['ActPanel']>[0]): JSX.Element {
  // split, not destructured: the panel follows `open` and the rest as they change
  const [own, rest] = splitProps(props, ['m']);
  return <Dynamic component={own.m.ActPanel} {...rest} />;
}

function WithMenu(props: { readonly act?: Act | undefined; readonly render: (trigger: () => Record<string, unknown>) => JSX.Element; readonly press: () => Record<string, unknown> }): JSX.Element {
  const press = props.press;
  return (
    <Show when={props.act?.menu} fallback={props.render(press)}>
      {(m) => (
        <MenuOn items={m().items} title={m().title} onSelect={(v) => m().onSelect(v)} onWarm={() => m().onWarm?.()} onOpenChange={(o) => o && m().onWarm?.()} part="ui.menu">
          {(trigger) => props.render(() => ({ ...trigger(), ...actAttrs(props.act) }))}
        </MenuOn>
      )}
    </Show>
  );
}

/** The icon, or the spinner that takes its place while pending (the same box). */
export function ActIcon(props: { readonly act?: Act | undefined; readonly children: JSX.Element }): JSX.Element {
  return (
    <Show when={props.act?.pending} fallback={props.children}>
      <Spinner in="act" />
    </Show>
  );
}
