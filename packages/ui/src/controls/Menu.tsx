// Menu and ContextMenu: the same entries feed both, so the right-click menu and More offer the same actions under the
// same words (design/ux.md). The trigger draws at once; Zag's menu machine (./menu-machine.tsx, with floating-ui) loads
// apart from the first page (./lazy-part.ts: on idle, or on the trigger's first hover, focus or press) and binds to it.
// A press before it's there shows the trigger pressed (aria-expanded) and opens the menu as it lands.
import { createEffect, createSignal, Show, untrack, type Accessor, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import type { ButtonSize, ButtonVariant } from './Button.tsx';
import { lazyPart, warm, type Replay } from './lazy-part.ts';

export { menuSections, type MenuEntry, type MenuGroup, type MenuItem, type MenuSeparator, type MenuSubmenu } from './menu-entries.ts';
import type { MenuEntry } from './menu-entries.ts';

const machine = lazyPart(() => import('./menu-machine.tsx'));

export type Placement = 'bottom-start' | 'bottom-end' | 'bottom' | 'top-start' | 'top-end' | 'top' | 'right-start' | 'left-start';

interface TriggerLook {
  readonly variant?: ButtonVariant;
  readonly size?: ButtonSize;
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


/** The trigger's props until the machine binds, then the machine's; and the machine itself, once it's loaded. */
function useLazyMenu(props: MenuBaseProps & { readonly placement?: Placement }, part: () => PartName, at?: 'pointer') {
  const [bound, setBound] = createSignal<Accessor<Record<string, unknown>>>();
  const [pending, setPending] = createSignal<Replay>();
  let area: HTMLElement | undefined;
  const press = (p: Replay): void => {
    setPending(p);
    machine.load().catch(() => setPending(undefined));
  };
  const early = warm(machine);
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
            press({ el: e.currentTarget as HTMLElement, type: 'contextmenu', x: e.clientX, y: e.clientY });
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
    <Show when={machine.ready()}>
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

export interface ContextMenuProps extends MenuBaseProps, WithChildren {}

/** Right-click (or long-press) on its children opens the menu at the pointer. */
export function ContextMenu(props: ContextMenuProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.context-menu';
  const m = useLazyMenu(props, part, 'pointer');
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
