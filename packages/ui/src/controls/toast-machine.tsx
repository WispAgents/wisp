// <Toasts>' machine half: Zag's toast group renders a toaster's stack, bottom centre. Loaded apart from the first page
// (./lazy-part.ts) with the store itself (the toaster creates it on its first toast). Zag's group machine does the stacking
// (only the newest in front, the rest tucked behind), pauses timers on hover and focus, and fans
// the stack out while pointed at. It sits above a pinned reply box: the shell sets
// `--toast-bottom` to the box's height so a toast never covers Send.
import * as toast from '@zag-js/toast';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createEffect, createMemo, createUniqueId, onCleanup, Show, type JSX } from 'solid-js';
import { ForKeyed } from '../core/keyed.tsx';
import { Portal } from 'solid-js/web';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { useT, type Translate } from '../core/i18n.tsx';
import { Icon } from '../content/Icon.tsx';
import { dockToasts } from './toast-dock-scope.ts';
import type { IconName } from '../content/icons.ts';

export { createStore } from '@zag-js/toast';

export interface ToastGroupProps extends KitProps {
  readonly store: toast.Store;
  readonly t?: Translate;
}

const GLYPH: Readonly<Record<string, IconName>> = { success: 'check', error: 'warn', info: 'info', loading: 'clock' };

const text = (v: unknown): string => (typeof v === 'string' ? v : '');

function ToastCard(props: {
  readonly toast: toast.Props;
  readonly index: number;
  readonly parent: toast.GroupService;
  readonly part: PartName;
  readonly t: Translate;
}): JSX.Element {
  // The store fills id, type, duration and removeDelay when it creates a toast; the fallbacks only
  // satisfy the machine's types.
  const machineProps = createMemo(() => ({
    ...props.toast,
    id: props.toast.id ?? '',
    type: props.toast.type ?? 'info',
    duration: props.toast.duration ?? 4000,
    removeDelay: props.toast.removeDelay ?? 200,
    parent: props.parent,
    index: props.index,
  }));
  const service = useMachine(toast.machine, machineProps);
  const api = createMemo(() => toast.connect(service, normalizeProps));
  return (
    <div {...api().getRootProps()} data-kit="toast" data-part={props.part} data-type={api().type}>
      <span {...api().getGhostBeforeProps()} />
      <div data-kit="toast-in">
        <span data-kit="toast-glyph">
          <Icon name={GLYPH[api().type] ?? 'info'} size="md" />
        </span>
        <div data-kit="toast-text">
          <div {...api().getTitleProps()} data-kit="toast-title">
            {text(api().title)}
          </div>
          <Show when={text(api().description)}>
            {(d) => (
              <div {...api().getDescriptionProps()} data-kit="toast-description">
                {d()}
              </div>
            )}
          </Show>
        </div>
        <Show when={props.toast.action}>
          {(action) => (
            <button {...api().getActionTriggerProps()} data-kit="toast-action" data-part={sub(props.part, 'action')}>
              {action().label}
            </button>
          )}
        </Show>
        <button
          {...api().getCloseTriggerProps()}
          aria-label={props.t('dismiss')}
          title={props.t('dismiss')}
          data-kit="toast-close"
          data-part={sub(props.part, 'close')}
        >
          <Icon name="x" size="sm" />
        </button>
      </div>
      <span {...api().getGhostAfterProps()} />
    </div>
  );
}

export function ToastGroup(props: ToastGroupProps): JSX.Element {
  const t = useT(() => props.t);
  const service = useMachine(toast.group.machine, () => ({ id: createUniqueId(), store: props.store }));
  const api = createMemo(() => toast.group.connect(service, normalizeProps));
  const part = (): PartName => props.part ?? 'ui.toast';
  // clear of the box at the bottom while toasts show (./toast-dock-scope.ts; v1 clearDock, B18)
  let group: HTMLDivElement | undefined;
  createEffect(() => {
    if (api().getToasts().length === 0 || !group) return;
    onCleanup(dockToasts(group));
  });
  return (
    <Portal>
      <div ref={(el) => (group = el)} {...api().getGroupProps({ label: t('notifications') })} data-kit="toasts" data-part={sub(part(), 'group')}>
        {/* by toast id: by position, a new toast moved every older one into another slot's machine and made the last
            one again, so a toast showing went and came back (qa/flicker.mjs) */}
        <ForKeyed each={api().getToasts()} by={(x) => x.id ?? ''}>
          {(item, id) => <ToastCard toast={item()} index={api().getToasts().findIndex((x) => x.id === id)} parent={service} part={part()} t={t} />}
        </ForKeyed>
      </div>
    </Portal>
  );
}
