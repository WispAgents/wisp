// Menu and ContextMenu: the same entries feed both, so the right-click menu and More offer the same actions under the
// same words (design/ux.md). The trigger draws at once; Zag's menu machine (./menu-machine.tsx, with floating-ui) loads
// apart from the first page (./lazy-part.ts: on idle, or on the trigger's first hover, focus or press) and binds to it.
// A press before it's there shows the trigger pressed (aria-expanded) and opens the menu as it lands.
// The phone apps (ADR-0065 build step 5, design/mobile.md §A.4, §A.7): a Menu from a button is the same machine in the
// platform's shape (M3 menu, iOS pull-down: the look's stylesheet); a ContextMenu opens on a long-press (the caller's
// gesture dispatches its contextmenu: Swipe's onLongPress) as Android's bottom sheet of the actions with *Select*
// first, or iOS's context menu with the pressed row as its preview (./phone.tsx, loaded apart: ./phone-part.ts).
import { createEffect, createSignal, Show, untrack, type Accessor, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import type { ButtonSize, ButtonVariant } from './Button.tsx';
import { lazyPart, warm, type Replay } from './lazy-part.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { phoneOverlays } from './phone-part.ts';

export { menuSections, type MenuEntry, type MenuGroup, type MenuItem, type MenuSeparator, type MenuSubmenu } from './menu-entries.ts';
import type { MenuEntry } from './menu-entries.ts';

const machine = lazyPart(() => import('./menu-machine.tsx'));

export type Placement = 'bottom-start' | 'bottom-end' | 'bottom' | 'top-start' | 'top-end' | 'top' | 'right-start' | 'left-start';

interface TriggerLook {
  readonly variant?: ButtonVariant;
  readonly size?: ButtonSize;
  /** shown in place of the icon (a team's faces, v1 .tb-stack). Additive (2026-11-17). */
  readonly lead?: JSX.Element;
}

/** What the menu's own button looks like: a label (and maybe an icon), or an icon with a name. */
export type MenuTrigger = (TriggerLook & { readonly label: string; readonly icon?: IconName }) | (TriggerLook & { readonly icon: IconName; readonly 'aria-label': string; readonly label?: undefined });

export interface MenuBaseProps extends KitProps {
  readonly items: readonly MenuEntry[];
  readonly onSelect: (value: string) => void;
  readonly open?: boolean;
  readonly onOpenChange?: (open: boolean) => void;
}

export interface MenuProps extends MenuBaseProps {
  readonly trigger: MenuTrigger;
  readonly placement?: Placement;
}


/** The trigger's props until the machine binds, then the machine's; and the machine itself, once it's loaded. `phone`:
 * a ContextMenu in the phone apps binds the phone chunk's stand-in (the same props) instead of Zag's. */
function useLazyMenu(props: MenuBaseProps & { readonly placement?: Placement }, part: () => PartName, at?: 'pointer', phone?: boolean) {
  const lazy = phone ? phoneOverlays : machine;
  const [bound, setBound] = createSignal<Accessor<Record<string, unknown>>>();
  const [pending, setPending] = createSignal<Replay>();
  let area: HTMLElement | undefined;
  const press = (p: Replay): void => {
    setPending(p);
    lazy.load().catch(() => setPending(undefined));
  };
  const early = warm(lazy);
  // controlled open: the machine follows `open` as soon as it's there
  createEffect(() => {
    if (props.open) early();
  });
  const plain = (): Record<string, unknown> =>
    at === 'pointer'
      ? {
          onPointerDown: early,
          onContextMenu: (e: MouseEvent) => {
            e.preventDefault();
            // the browser replays on the area, as before; a phone on what was pressed (its menu reads the row)
            press({ el: (phone && e.target instanceof HTMLElement ? e.target : e.currentTarget) as HTMLElement, type: 'contextmenu', x: e.clientX, y: e.clientY });
          },
        }
      : {
          type: 'button',
          'aria-haspopup': 'menu',
          'aria-expanded': pending() !== undefined || props.open === true,
          'data-state': pending() !== undefined || props.open === true ? 'open' : 'closed',
          onPointerEnter: early,
          onFocus: early,
          onPointerDown: early,
          onClick: (e: MouseEvent) => press({ el: e.currentTarget as HTMLElement, type: 'click' }),
          onKeyDown: (e: KeyboardEvent) => {
            if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
            e.preventDefault();
            press({ el: e.currentTarget as HTMLElement, type: 'keydown', key: e.key });
          },
        };
  const trigger = (): Record<string, unknown> => bound()?.() ?? plain();
  const content = (): JSX.Element => (
    <Show when={lazy.ready()}>
      {(m) => {
        const M = m().MenuMachine;
        const p = untrack(pending);
        return <M base={props} part={part()} bind={(b) => setBound(() => b)} pending={p} {...(at ? { at, area: () => area } : {})} />;
      }}
    </Show>
  );
  return { trigger, content, setArea: (el: HTMLElement) => (area = el) };
}

const labelOf = (t: MenuTrigger): string | undefined => ('aria-label' in t ? t['aria-label'] : undefined);

/** A button that opens a list of actions. */
export function Menu(props: MenuProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.menu';
  const m = useLazyMenu(props, part);
  const t = (): MenuTrigger => props.trigger;
  return (
    <>
      <button
        {...m.trigger()}
        aria-label={labelOf(t())}
        title={labelOf(t())}
        data-kit={t().label ? 'button' : 'icon-button'}
        data-part={sub(part(), 'trigger')}
        data-variant={t().variant ?? (t().label ? 'secondary' : 'ghost')}
        data-size={t().size ?? 'md'}
      >
        <span data-kit="button-content">
          <Show when={t().icon}>{(name) => <Icon name={name()} size={t().label ? 'md' : 'base'} />}</Show>
          <Show when={t().label}>
            {(label) => (
              <span data-kit="button-label" data-text="control">
                {label()}
              </span>
            )}
          </Show>
        </span>
      </button>
      {m.content()}
    </>
  );
}

export interface ContextMenuProps extends MenuBaseProps, WithChildren {
  /** the phone apps: the list's selection mode, offered first in Android's long-press sheet (design/mobile.md §A.4) */
  readonly onSelectMode?: () => void;
}

/** Right-click (or long-press) on its children opens the menu at the pointer. */
export function ContextMenu(props: ContextMenuProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.context-menu';
  const m = useLazyMenu(props, part, 'pointer', usePlatform()() !== 'web');
  return (
    <>
      <div {...m.trigger()} ref={m.setArea} data-kit="context-area" data-part={sub(part(), 'area')}>
        {props.children}
      </div>
      {m.content()}
    </>
  );
}

export interface MenuOnProps extends MenuBaseProps {
  readonly placement?: Placement;
  /** draws the control that opens the menu: spread `trigger()` on it (Zag's trigger props: its press,
   * aria-expanded, the id the menu points back at) */
  readonly children: (trigger: () => Record<string, unknown>) => JSX.Element;
}

/** A menu opened by a control drawn by its caller (the app frame's v1 pickers), not by the kit's button. */
export function MenuOn(props: MenuOnProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.menu';
  const m = useLazyMenu(props, part);
  return (
    <>
      {props.children(m.trigger)}
      {m.content()}
    </>
  );
}
