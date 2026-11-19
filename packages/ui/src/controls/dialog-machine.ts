// Dialog and Sheet's machine half (Zag's dialog): focus moves in and is trapped, Escape and the backdrop close it, focus
// returns to what opened it, the page behind doesn't scroll. Loaded apart from the first page (./lazy-part.ts) and bound
// to the surface Dialog.tsx draws at once.
import * as dialog from '@zag-js/dialog';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createEffect, createMemo, createUniqueId, onCleanup, type Accessor } from 'solid-js';
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

// how many open dialogs hide each node, and the ones that were hidden before any did (left as they were on close)
const hiders = new WeakMap<Element, number>();
const hiddenBefore = new WeakSet<Element>();
const ignored = (el: Element): boolean =>
  el.localName === 'script' || el.localName === 'output' || el.getAttribute('role') === 'status' || el.hasAttribute('aria-live') || el.hasAttribute('data-live-announcer');

/**
 * What a screen reader may read while a dialog is open: everything outside `content` is `aria-hidden` (Zag's
 * hideOthers, on the same nodes: each sibling on the way from <body> down to it). Done here because Zag ties it to
 * `modal`, and a modal dialog also blocks the page's pointer with `pointer-events: none` on <body>: an inherited
 * property, so the whole document restyled as ⌘K or a dialog opened and again as it closed (perf-ui-r2 #6: 1,526–1,792
 * elements; aria-hidden alone restyles nothing). The backdrop covers the page, so the pointer needs no block. Returns
 * the undo. Exported for its test.
 */
export function hideOutside(content: Element): () => void {
  const keep = new Set<Element>();
  for (let el: Element | null = content; el; el = el.parentElement) keep.add(el);
  const hidden: Element[] = [];
  const walk = (parent: Element): void => {
    for (const el of parent.children) {
      if (el === content) continue;
      if (keep.has(el)) {
        walk(el);
        continue;
      }
      if (ignored(el)) continue;
      const n = hiders.get(el) ?? 0;
      if (n === 0 && el.getAttribute('aria-hidden') === 'true') hiddenBefore.add(el);
      hiders.set(el, n + 1);
      if (n === 0) el.setAttribute('data-aria-hidden', '');
      if (n === 0 && !hiddenBefore.has(el)) el.setAttribute('aria-hidden', 'true');
      hidden.push(el);
    }
  };
  const body = content.ownerDocument.body;
  if (keep.has(body)) walk(body);
  return () => {
    for (const el of hidden) {
      const n = (hiders.get(el) ?? 1) - 1;
      hiders.set(el, n);
      if (n > 0) continue;
      if (!hiddenBefore.has(el)) el.removeAttribute('aria-hidden');
      hiddenBefore.delete(el);
      el.removeAttribute('data-aria-hidden');
    }
  };
}

/** A style (Solid's object or a string) without Zag's non-modal `pointer-events: none`. */
function withoutPointerNone(style: unknown): unknown {
  if (typeof style === 'string') return style.replace(/(^|;)\s*pointer-events:\s*none\s*;?/, '$1').trim();
  if (style && typeof style === 'object') {
    const { 'pointer-events': _pe, pointerEvents: _pe2, ...rest } = style as Record<string, unknown>;
    return rest;
  }
  return style;
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
    // modal in all but Zag's word (hideOutside): focus trapped, the page hidden from screen readers, aria-modal
    modal: false,
    trapFocus: true,
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
  const api = createMemo(() => {
    const a = dialog.connect(service, normalizeProps);
    // as a modal one draws: aria-modal on the content, and the positioner takes the pointer (Zag lets it through when
    // not modal), so a press outside the content lands on it and closes the dialog as before
    return {
      ...a,
      getContentProps: () => ({ ...a.getContentProps(), 'aria-modal': 'true' }),
      getPositionerProps: () => {
        const p = a.getPositionerProps() as Record<string, unknown>;
        return { ...p, style: withoutPointerNone(p['style']) };
      },
    };
  });
  // once the content is drawn (the frame after it opens, as Zag did), the rest of the page goes quiet
  const open = createMemo(() => api().open);
  createEffect(() => {
    if (!open()) return;
    let undo: (() => void) | undefined;
    const f = requestAnimationFrame(() => {
      const el = document.getElementById(String(api().getContentProps()['id'] ?? ''));
      if (el?.isConnected) undo = hideOutside(el);
    });
    onCleanup(() => {
      cancelAnimationFrame(f);
      undo?.();
    });
  });
  return api;
}
