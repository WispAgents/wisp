// Popover: a floating card anchored to its button. Outside click and Escape close it and put
// focus back on the button (design/ux.md).
// The phone apps (ADR-0065 build step 5, design/mobile.md §A.7): on a phone it becomes a bottom sheet (the kit's
// Sheet: M3 modal bottom sheet, the iOS sheet with detents), with the same trigger, title and body; an iPad (600 pt
// and up) keeps the popover. The browser's is what it was.
import * as popover from '@zag-js/popover';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, createSignal, createUniqueId, Show, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { styleOf, sub, type KitProps, type PartName, type WithChildren, type WithVars } from '../core/props.ts';
import { useT, type Translate } from '../core/i18n.tsx';
import { Icon } from '../content/Icon.tsx';
import type { MenuTrigger, Placement } from './Menu.tsx';
import { TriggerButton, triggerName } from './trigger.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { Sheet } from './Dialog.tsx';

export interface PopoverProps extends KitProps, WithChildren, WithVars {
  readonly trigger: MenuTrigger;
  /** A heading inside the card; with one, the card also gets a close button. */
  readonly title?: string;
  readonly placement?: Placement;
  readonly open?: boolean;
  readonly onOpenChange?: (open: boolean) => void;
  /** Keep focus inside while open (for small forms). */
  readonly modal?: boolean;
  /** No padding of its own: the content is a panel with its head and foot edge to edge (TeamPop). Additive
   *  (2026-11-16). */
  readonly bare?: boolean;
  /** Kept inside the window 12 px from its edges, not inside the boxes around its button (v1's team panel: left =
   *  min(button's left, window − 12 − width)). Additive (2026-11-17). */
  readonly inWindow?: boolean;
  /** One more part name on the card (v1's PARTS name for what it is: `delegate.panel` for a team panel). Additive. */
  readonly cardPart?: PartName;
  readonly t?: Translate;
}

/** iPad widths keep a popover (HIG); a phone gets a sheet */
const wide = (): boolean => typeof window !== 'undefined' && window.matchMedia?.('(min-width: 600px)').matches === true;

export function Popover(props: PopoverProps): JSX.Element {
  const look = usePlatform();
  return (
    <Show when={look() === 'android' || (look() === 'ios' && !wide())} fallback={<Floating {...props} />}>
      <AsSheet {...props} />
    </Show>
  );
}

function AsSheet(props: PopoverProps): JSX.Element {
  const [open, setOpen] = createSignal(false);
  return (
    <Sheet
      open={props.open ?? open()}
      onOpenChange={(o) => {
        setOpen(o);
        props.onOpenChange?.(o);
      }}
      trigger={props.trigger}
      // a sheet needs a name: the trigger's when the popover has no title
      title={props.title ?? props.trigger.label ?? triggerName(props.trigger) ?? ''}
      part={props.part ?? 'ui.popover'}
      {...(props.vars ? { vars: props.vars } : {})}
      {...(props.t ? { t: props.t } : {})}
    >
      {props.children}
    </Sheet>
  );
}

function Floating(props: PopoverProps): JSX.Element {
  const t = useT(() => props.t);
  const id = createUniqueId();
  const service = useMachine(popover.machine, () => ({
    id,
    ...(props.open === undefined ? {} : { open: props.open }),
    onOpenChange: (d: popover.OpenChangeDetails) => props.onOpenChange?.(d.open),
    positioning: { placement: props.placement ?? 'bottom-start', gutter: 6, ...(props.inWindow && { boundary: () => document.documentElement, overflowPadding: 12, flip: false }) },
    ...(props.modal === undefined ? {} : { modal: props.modal }),
    // a harness picker opened from inside it lives on the page (HarnessPicker): a click there isn't outside the card
    persistentElements: [() => document.querySelector<HTMLElement>('[data-kit="picker"]')],
  }));
  const api = createMemo(() => popover.connect(service, normalizeProps));
  const part = (): PartName => props.part ?? 'ui.popover';
  return (
    <>
      <TriggerButton look={props.trigger} bind={api().getTriggerProps()} part={sub(part(), 'trigger')} />
      <Portal>
        <div {...api().getPositionerProps()} data-kit="positioner">
          <div {...api().getContentProps()} data-kit="popover" data-part={props.cardPart ? `${part()} ${props.cardPart}` : part()} data-bare={props.bare ? '' : undefined} style={styleOf({}, props.vars)}>
            <Show when={props.title}>
              {(title) => (
                <div data-kit="popover-head">
                  <div {...api().getTitleProps()} data-kit="popover-title" data-part={sub(part(), 'title')}>
                    {title()}
                  </div>
                  <button {...api().getCloseTriggerProps()} aria-label={t('close')} title={t('close')} data-kit="icon-button" data-variant="ghost" data-size="xs">
                    <Icon name="x" size="sm" />
                  </button>
                </div>
              )}
            </Show>
            <div data-kit="popover-body" data-part={sub(part(), 'body')}>
              {props.children}
            </div>
          </div>
        </div>
      </Portal>
    </>
  );
}
