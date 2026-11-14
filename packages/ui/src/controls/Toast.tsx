// <Toasts>: renders a toaster's stack, bottom centre (./toast-machine.tsx: Zag's group machine stacks them, pauses
// timers on hover and focus, and fans the stack out while pointed at). It sits above a pinned reply box: the shell sets
// `--toast-bottom` to the box's height so a toast never covers Send. The machine loads apart from the first page, with
// the toaster's store, on the first toast (or once the first screen is up: ./lazy-part.ts preloadControls).
import { Show, type JSX } from 'solid-js';
import type { KitProps } from '../core/props.ts';
import type { Translate } from '../core/i18n.tsx';
import { toastMachine, type Toaster } from './toaster.ts';

export interface ToastsProps extends KitProps {
  readonly toaster: Toaster;
  readonly t?: Translate;
}

export function Toasts(props: ToastsProps): JSX.Element {
  return (
    <Show when={toastMachine.ready()}>
      {(m) => {
        const Group = m().ToastGroup;
        return <Show when={props.toaster.store()}>{(store) => <Group store={store()} {...(props.t ? { t: props.t } : {})} {...(props.part ? { part: props.part } : {})} />}</Show>;
      }}
    </Show>
  );
}
