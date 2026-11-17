// Dialog and Sheet on Zag's dialog machine: focus moves in and is trapped, Escape and the
// backdrop close it, focus returns to what opened it, the page behind doesn't scroll. It opens in
// its final shape at once (design/ux.md): the caller shows placeholders for what still loads.
// The machine (./dialog-machine.ts) loads apart from the first page (./lazy-part.ts); until it's bound, the surface
// draws with plain props (its role, Escape, the backdrop and × close it) and the machine takes it over as it lands.
// The phone apps (ADR-0065 build step 5, design/mobile.md §A.7): Android draws an M3 basic dialog (28 dp, its buttons
// end-aligned text buttons) and the Sheet as an M3 modal bottom sheet (drag handle, 28 dp top corners); iOS draws an
// alert (270 pt, the destructive action red, Cancel bold) for a confirm (`alertdialog` or `sm`), and every other
// dialog as a sheet with detents (medium, large), a grabber and 10 pt corners. A phone sheet drags down to close (a
// draft stays; its × is the twin: step 7's `sheet.drag-down`); the drag lives in the phone chunk (./phone-part.ts).
// The browser's HTML is what it was.
import { createEffect, createSignal, onCleanup, Show, untrack, type Accessor, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { lazyPart, warm } from './lazy-part.ts';
import type { DialogApi } from './dialog-machine.ts';
import { styleOf, sub, type KitProps, type PartName, type WithChildren, type WithVars } from '../core/props.ts';
import { useT, type Translate } from '../core/i18n.tsx';
import { Icon } from '../content/Icon.tsx';
import type { MenuTrigger } from './Menu.tsx';
import { TriggerButton } from './trigger.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useGestureHost } from '../gestures/host.tsx';
import { phoneOverlays } from './phone-part.ts';
import type { Detent } from './phone-math.ts';

/** v1: v1's .modal (640 wide, 18px 20px 20px, its 600 18px title): the crew editor */
/** window: v1's reader and app window (.dr, .ap), 18px in from the window's edges, the whole screen on a phone */
export type DialogSize = 'sm' | 'md' | 'lg' | 'xl' | 'v1' | 'full' | 'window';

export interface DialogProps extends KitProps, WithChildren, WithVars {
  readonly title: string;
  readonly description?: string;
  /** Buttons along the bottom: Cancel, then the action named for what it does. */
  readonly footer?: JSX.Element;
  /** Controlled: the caller owns whether it's open. */
  readonly open?: boolean;
  readonly onOpenChange?: (open: boolean) => void;
  /** Or let the dialog render its own opening button. */
  readonly trigger?: MenuTrigger;
  /** 400, 600 (default) or 800 wide; xl: v1's Settings modal, 900 × 680 (less on a short window); full: the whole
   *  screen (the phone's crew story, whose stylesheet draws it: crew-stories.css) */
  readonly size?: DialogSize;
  /** `alertdialog` for a confirm that interrupts. */
  readonly role?: 'dialog' | 'alertdialog';
  /** Clicking outside closes it (default true; false for a form with unsaved input). */
  readonly closeOnOutside?: boolean;
  /** the content brings its own head and close (a frame like Settings): the title stays for screen readers only */
  readonly bare?: boolean;
  /** 'top': near the top over a light scrim, the content its own box (v1's ⌘K palette) */
  readonly place?: 'top';
  /** Where focus goes first (default: the first focusable inside). */
  readonly initialFocus?: () => HTMLElement | null;
  readonly t?: Translate;
  /** iOS sheets only: the heights it rests at, opening at the first (default medium and large) */
  readonly detents?: readonly Detent[];
}

type Kind = 'dialog' | 'sheet';

const machine = lazyPart(() => import('./dialog-machine.ts'));

type Bind = Record<string, unknown>;

/** The surface's parts' props: the machine's once it's bound, plain ones before. */
interface Parts {
  /** no machine yet: the surface moves focus in itself */
  plain(): boolean;
  backdrop(): Bind;
  positioner(): Bind;
  content(): Bind;
  title(): Bind;
  description(): Bind;
  close(): Bind;
}

function partsOf(api: () => DialogApi | undefined, props: DialogProps, close: () => void): Parts {
  const outside = (e: PointerEvent): void => {
    if (e.target === e.currentTarget && (props.closeOnOutside ?? true)) close();
  };
  const keys = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    close();
  };
  return {
    plain: () => api() === undefined,
    backdrop: () => api()?.getBackdropProps() ?? { 'data-state': 'open' },
    positioner: () => api()?.getPositionerProps() ?? { onPointerDown: outside },
    content: () => api()?.getContentProps() ?? { role: props.role ?? 'dialog', 'aria-modal': 'true', tabIndex: -1, 'data-state': 'open', onKeyDown: keys },
    title: () => api()?.getTitleProps() ?? {},
    description: () => api()?.getDescriptionProps() ?? {},
    close: () => api()?.getCloseTriggerProps() ?? { type: 'button', onClick: close },
  };
}

function Surface(props: DialogProps & { readonly kind: Kind; readonly parts: Parts; readonly part: PartName; readonly dismiss: () => void }): JSX.Element {
  const t = useT(() => props.t);
  const look = usePlatform();
  const host = useGestureHost();
  let stop: (() => void) | undefined;
  onCleanup(() => stop?.());
  // the phone apps: the platform's shape (an alert, a sheet's drag and detents: the phone chunk, ./phone.tsx)
  const bind = (el: HTMLElement): void => {
    const l = look();
    if (l !== 'web')
      void phoneOverlays.load().then((m) => {
        if (el.isConnected) stop = m.phoneSurface(el, { host, look: l, kind: props.kind, role: props.role, size: props.size, bare: props.bare, place: props.place, detents: props.detents, part: props.part, onClose: props.dismiss, closeLabel: t('close') });
      }, () => undefined);
  };
  return (
    <Portal>
      <div {...props.parts.backdrop()} data-kit="backdrop" data-sheet={props.kind === 'sheet' ? '' : undefined} data-place={props.place} />
      <div {...props.parts.positioner()} data-kit="dialog-positioner" data-sheet={props.kind === 'sheet' ? '' : undefined} data-size={props.size ?? 'md'} data-place={props.place}>
        <div
          {...props.parts.content()}
          // shown before its machine: focus goes in (the machine traps it once it lands), so Escape and typing reach it
          ref={(el) => {
            if (props.parts.plain()) queueMicrotask(() => (props.initialFocus?.() ?? el).focus());
            bind(el);
          }}
          data-kit={props.kind}
          data-part={props.part}
          data-size={props.size ?? 'md'}
          data-bare={props.bare ? '' : undefined}
          data-place={props.place}
          style={styleOf({}, props.vars)}
        >
          <Show when={props.kind === 'sheet'}>
            <span data-kit="sheet-handle" aria-hidden="true" />
          </Show>
          <header data-kit="dialog-head" data-part={sub(props.part, 'head')}>
            <h2 {...props.parts.title()} data-kit="dialog-title" data-part={sub(props.part, 'title')}>
              {props.title}
            </h2>
            <button
              {...props.parts.close()}
              aria-label={t('close')}
              title={t('close')}
              data-kit="icon-button"
              data-variant="ghost"
              data-size="sm"
              data-part={sub(props.part, 'close')}
            >
              <Icon name="x" size="md" />
            </button>
          </header>
          <Show when={props.description}>
            {(text) => (
              <p {...props.parts.description()} data-kit="dialog-description">
                {text()}
              </p>
            )}
          </Show>
          <div data-kit="dialog-body" data-part={sub(props.part, 'body')}>
            {props.children}
          </div>
          <Show when={props.footer}>
            <footer data-kit="dialog-foot" data-part={sub(props.part, 'foot')}>
              {props.footer}
            </footer>
          </Show>
        </div>
      </div>
    </Portal>
  );
}

/** Where focus goes back as the surface closes: its own trigger, else what had focus as it opened. The kit gives it back
 * itself, in the same task the surface goes: Zag's focus trap remembers what was focused when it starts (a frame after
 * opening; one opened before its machine has moved focus in by then) and gives it back on a timer, so focus sat on
 * <body> for a while, or for good (dialog-focus-return.test.tsx). */
function focusReturn(open: Accessor<boolean>): { readonly to: () => HTMLElement | null; readonly trigger: (el: HTMLElement) => void } {
  let triggerEl: HTMLElement | undefined;
  let opener: HTMLElement | undefined;
  const to = (): HTMLElement | null => [triggerEl, opener].find((el) => el?.isConnected) ?? null;
  let was = false;
  createEffect(() => {
    const now = open();
    const a = document.activeElement;
    if (now && !was) opener = a instanceof HTMLElement && a !== document.body ? a : undefined;
    else if (!now && was) {
      // focus was in the surface (now gone) or nowhere; one the user moved elsewhere (a click on a field outside) stays
      const el = to();
      if (el && (!a || a === document.body || !a.isConnected)) el.focus({ preventScroll: true });
    }
    was = now;
  });
  return { to, trigger: (el) => (triggerEl = el) };
}

function Shell(props: DialogProps & { readonly kind: Kind }): JSX.Element {
  const [api, setApi] = createSignal<Accessor<DialogApi>>();
  // opened by its own trigger before the machine was there
  const [selfOpen, setSelfOpen] = createSignal(false);
  const bound = (): DialogApi | undefined => api()?.();
  const open = (): boolean => bound()?.open ?? props.open ?? selfOpen();
  const focus = focusReturn(open);
  createEffect(() => {
    if (open()) warm(machine)();
  });
  const close = (): void => {
    if (props.open === undefined) setSelfOpen(false);
    props.onOpenChange?.(false);
  };
  const parts = partsOf(bound, props, close);
  // a phone sheet dragged down: through the machine when it's there (focus goes back, the scroll lock lifts)
  const dismiss = (): void => {
    const a = bound();
    if (a) a.setOpen(false);
    else close();
  };
  const part = (): PartName => props.part ?? (props.kind === 'sheet' ? 'ui.sheet' : 'ui.dialog');
  const trigger = (): Bind =>
    bound()?.getTriggerProps() ?? {
      'aria-haspopup': 'dialog',
      'aria-expanded': selfOpen(),
      onPointerEnter: warm(machine),
      onFocus: warm(machine),
      onClick: () => {
        setSelfOpen(true);
        props.onOpenChange?.(true);
      },
    };
  return (
    <>
      <Show when={machine.ready()}>
        {(m) => {
          setApi(() => m().useDialog(props, untrack(selfOpen), focus.to));
          return null;
        }}
      </Show>
      <Show when={props.trigger}>
        {(look) => <TriggerButton look={look()} bind={trigger()} part={sub(part(), 'trigger')} ref={focus.trigger} />}
      </Show>
      <Show when={open()}>
        <Surface {...props} parts={parts} part={part()} dismiss={dismiss} />
      </Show>
    </>
  );
}

/** A centred card over a dimmed page. */
export function Dialog(props: DialogProps): JSX.Element {
  return <Shell {...props} kind="dialog" />;
}

/** On phones a sheet from the bottom with a grab handle; from the phone breakpoint up, a card. */
export function Sheet(props: DialogProps): JSX.Element {
  return <Shell {...props} kind="sheet" />;
}
