// Tooltip: a short label on hover or keyboard focus. It wraps its children in a span that carries
// the trigger props; Zag's Solid adapter turns onFocus/onBlur into focusin/focusout, which bubble
// from the focused child, so keyboard focus opens it too.
// The phone apps (ADR-0065 build step 5, design/mobile.md §A.7): no hover there. Android shows the M3 plain tooltip on a
// long-press of the trigger (1.5 s; that press opens nothing: ./phone.tsx bindTip); iOS has none (HIG). Neither binds
// the machine. The browser's is what it was.
import * as tooltip from '@zag-js/tooltip';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, createUniqueId, onCleanup, Show, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useGestureHost } from '../gestures/host.tsx';
import { phoneOverlays } from './phone-part.ts';

export interface TooltipProps extends KitProps, WithChildren {
  readonly content: string;
  /** A second, quieter line. */
  readonly hint?: string;
  readonly placement?: 'top' | 'bottom' | 'left' | 'right';
  /** ms before it shows (default 400). */
  readonly openDelay?: number;
  readonly disabled?: boolean;
  /** Held open by the caller (a click's confirmation: Copied), hover or not; with `disabled`, only then. */
  readonly open?: boolean;
}

export function Tooltip(props: TooltipProps): JSX.Element {
  const look = usePlatform();
  return (
    <Show when={look() === 'web'} fallback={<OnPhone {...props} />}>
      <Hover {...props} />
    </Show>
  );
}

function OnPhone(props: TooltipProps): JSX.Element {
  const look = usePlatform();
  const host = useGestureHost();
  let stop: (() => void) | undefined;
  onCleanup(() => stop?.());
  const bind = (el: HTMLElement): void => {
    if (look() !== 'android') return;
    void phoneOverlays.load().then((m) => {
      if (el.isConnected && !props.disabled) stop = m.bindTip(el, { host, label: () => props.content, hint: () => props.hint });
    }, () => undefined);
  };
  return (
    <span ref={bind} data-kit="tooltip-trigger" data-part={sub(props.part ?? 'ui.tooltip', 'trigger')}>
      {props.children}
    </span>
  );
}

function Hover(props: TooltipProps): JSX.Element {
  const id = createUniqueId();
  const service = useMachine(tooltip.machine, () => ({
    id,
    openDelay: props.openDelay ?? 400,
    closeDelay: 100,
    positioning: { placement: props.placement ?? 'top', gutter: 6 },
    ...(props.disabled === undefined ? {} : { disabled: props.disabled }),
    ...(props.open === undefined ? {} : { open: props.open }),
  }));
  const api = createMemo(() => tooltip.connect(service, normalizeProps));
  const part = (): PartName => props.part ?? 'ui.tooltip';
  // Zag types the trigger as a button; its props (pointer, focus, aria) are as valid on a span.
  const trigger = (): JSX.HTMLAttributes<HTMLSpanElement> => api().getTriggerProps() as JSX.HTMLAttributes<HTMLSpanElement>;
  return (
    <>
      <span {...trigger()} data-kit="tooltip-trigger" data-part={sub(part(), 'trigger')}>
        {props.children}
      </span>
      <Show when={api().open}>
        <Portal>
          <div {...api().getPositionerProps()} data-kit="tooltip-positioner">
            <div {...api().getContentProps()} data-kit="tooltip" data-part={part()}>
              <span>{props.content}</span>
              <Show when={props.hint}>{(hint) => <span data-kit="tooltip-hint">{hint()}</span>}</Show>
            </div>
          </div>
        </Portal>
      </Show>
    </>
  );
}
