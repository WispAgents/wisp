// Dialog and Sheet's machine half (Zag's dialog): focus moves in and is trapped, Escape and the backdrop close it, focus
// returns to what opened it, the page behind doesn't scroll. Loaded apart from the first page (./lazy-part.ts) and bound
// to the surface Dialog.tsx draws at once.
import * as dialog from '@zag-js/dialog';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, createUniqueId, type Accessor } from 'solid-js';
import type { DialogProps } from './Dialog.tsx';

export type DialogApi = dialog.Api;

/**
 * Whether the page itself scrolls. wisp's never does (every scrolling box is inside the frame), so a dialog has no page
 * to hold still, and Zag's scroll lock would only cost: it writes `--scrollbar-width` on <html>, an inherited custom
 * property, which restyles the whole document as the dialog opens and again as it closes (perf audit 2026-11-18 #9:
 * 1,600 elements at ⌘K). A page that does scroll (the gallery) keeps the lock.
 */
export function pageScrolls(doc: Document = document): boolean {
  const el = doc.scrollingElement;
  return !!el && el.scrollHeight > el.clientHeight;
}

/** `open0`: it was opened (by its own trigger) before the machine was there. `returnTo`: where focus goes back on close
 * (the kit's trigger carries its own data-part, so Zag's lookup by `data-part="trigger"` doesn't find it). */
export function useDialog(props: DialogProps, open0: boolean, returnTo: () => HTMLElement | null): Accessor<dialog.Api> {
  const id = createUniqueId();
  // read once, as the machine is made: in the props below it would read layout again each time `open` changes
  const lock = pageScrolls();
  const service = useMachine(dialog.machine, () => ({
    id,
    ...(props.open === undefined ? { defaultOpen: open0 } : { open: props.open }),
    onOpenChange: (d: dialog.OpenChangeDetails) => props.onOpenChange?.(d.open),
    role: props.role ?? 'dialog',
    preventScroll: lock,
    closeOnInteractOutside: props.closeOnOutside ?? true,
    // Escape in a search field with text in it clears the field (its own handler, or the browser's), and the dialog
    // stays: the next Escape closes it, as in every OS's settings
    onEscapeKeyDown: (e: KeyboardEvent) => {
      const t = e.target;
      if (t instanceof HTMLInputElement && t.type === 'search' && t.value !== '') e.preventDefault();
    },
    ...(props.initialFocus ? { initialFocusEl: props.initialFocus } : {}),
    finalFocusEl: returnTo,
  }));
  return createMemo(() => dialog.connect(service, normalizeProps));
}
