// Dialog and Sheet's machine half (Zag's dialog): focus moves in and is trapped, Escape and the backdrop close it, focus
// returns to what opened it, the page behind doesn't scroll. Loaded apart from the first page (./lazy-part.ts) and bound
// to the surface Dialog.tsx draws at once.
import * as dialog from '@zag-js/dialog';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, createUniqueId, type Accessor } from 'solid-js';
import type { DialogProps } from './Dialog.tsx';

export type DialogApi = dialog.Api;

/** `open0`: it was opened (by its own trigger) before the machine was there. `returnTo`: where focus goes back on close
 * (the kit's trigger carries its own data-part, so Zag's lookup by `data-part="trigger"` doesn't find it). */
export function useDialog(props: DialogProps, open0: boolean, returnTo: () => HTMLElement | null): Accessor<dialog.Api> {
  const id = createUniqueId();
  const service = useMachine(dialog.machine, () => ({
    id,
    ...(props.open === undefined ? { defaultOpen: open0 } : { open: props.open }),
    onOpenChange: (d: dialog.OpenChangeDetails) => props.onOpenChange?.(d.open),
    role: props.role ?? 'dialog',
    closeOnInteractOutside: props.closeOnOutside ?? true,
    ...(props.initialFocus ? { initialFocusEl: props.initialFocus } : {}),
    finalFocusEl: returnTo,
  }));
  return createMemo(() => dialog.connect(service, normalizeProps));
}
