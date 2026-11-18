// A submenu as a flyout beside its row, for a desktop pointer (owner 2026-11-17): the standard cascading menu, on Zag's
// nested menus. The row is Zag's trigger item for a child menu machine, so hover intent (Zag's 200 ms), → / Enter / a
// click to open, ← to close, right-start placement flipped left at the screen's edge and slid to stay on it, and a choice
// closing the whole menu are all Zag's. Two things are the kit's: the flyout's open state is controlled, so Zag's close
// 100 ms after the pointer leaves the row becomes a request held while the pointer is still heading for the flyout (the
// safe triangle, ./menu-aim.ts); and Escape closes only the flyout, where Zag's would close the whole menu.
// Loaded with ./menu-machine.tsx, apart from the first page.
import * as menu from '@zag-js/menu';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, createSignal, createUniqueId, onCleanup, onMount, Show, type Accessor, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { sub, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { Look } from '../platform/look.ts';
import type { MenuEntry, MenuSubmenu } from './menu-entries.ts';
import { aimTriangle, emptyBox, inBox, inTriangle, type Box, type Point } from './menu-aim.ts';
import { nextTask, useTimeout } from '../blocks/rich-scope.ts';

/** Submenus fly out for a hovering fine pointer in the browser; touch screens and the phone apps drill in. */
export function flyoutPointer(look: Look): boolean {
  const mq = globalThis.matchMedia;
  return look === 'web' && typeof mq === 'function' && mq('(hover: hover) and (pointer: fine)').matches;
}

/** What a flyout needs from the menu it hangs off. */
export interface FlyContext {
  readonly parent: menu.Service;
  readonly onSelect: (value: string) => void;
  /** keeps the flyout's close, called when its menu closes or re-anchors; returns its unregister */
  readonly adopt: (close: () => void) => () => void;
  /** draws a list of entries (the machine's own Entries, handed in so the two files don't import each other) */
  readonly Entries: (p: { readonly api: menu.Api; readonly items: readonly MenuEntry[]; readonly part: PartName }) => JSX.Element;
}

/** Resting inside the triangle without reaching the flyout: it closes after this. */
const GRACE_MS = 300;
/** An open or close this soon after a key press was the keyboard's: no aiming to wait for. */
const KEYED_MS = 100;

const boxOf = (el: Element | undefined): Box | undefined => {
  const r = el?.getBoundingClientRect();
  return r && !emptyBox(r) ? r : undefined;
};

const firstValue = (items: MenuSubmenu['items']): string | undefined => {
  for (const e of items) {
    if (e.kind === 'separator') continue;
    if (e.kind === 'group') {
      const x = e.items.find((i) => !i.disabled);
      if (x) return x.value;
    } else if (!e.disabled) return e.value;
  }
  return undefined;
};

interface OpenDeps {
  readonly row: () => HTMLElement | undefined;
  readonly content: () => HTMLElement | undefined;
  /** the pointer is back on the row: Zag's child back to plain open (its own leave-tracking off) */
  readonly reopen: () => void;
  /** Escape in the flyout: close it as ← does */
  readonly back: () => void;
  readonly keyedOpen: () => void;
}

/** The flyout's open state. Zag asks; this decides, holding a close while the pointer aims at the flyout. */
function useFlyoutOpen(deps: OpenDeps): { readonly open: Accessor<boolean>; readonly request: (open: boolean) => void; readonly close: () => void } {
  const [open, setOpen] = createSignal(false);
  const grace = useTimeout();
  const aim: Aim = { from: undefined, last: undefined, holding: false, keyAt: -Infinity };
  const close = (): void => {
    aim.holding = false;
    aim.from = undefined;
    grace.stop();
    setOpen(false);
  };
  const hold = (): void => {
    aim.holding = true;
    grace.start(GRACE_MS, close);
  };
  const request = (want: boolean): void => {
    const keyed = performance.now() - aim.keyAt < KEYED_MS;
    const at = keyed ? undefined : aim.last;
    if (want) {
      setOpen(true);
      if (keyed) deps.keyedOpen();
    } else if (at && onBox(at, deps.row())) deps.reopen();
    else if (at && aiming(aim, at, deps.content())) hold();
    else close();
  };
  usePointerAndKeys(deps, aim, { open, hold, close, stopHold: () => grace.stop() });
  return { open, request, close };
}

/** Where the pointer was, and whether a close is being held for it. */
interface Aim {
  from: Point | undefined; // where the pointer left the row, while it may be on its way to the flyout
  last: Point | undefined;
  holding: boolean;
  keyAt: number;
}

const onBox = (p: Point, el: Element | undefined): boolean => {
  const b = boxOf(el);
  return !!b && inBox(p, b);
};

const aiming = (aim: Aim, p: Point, content: Element | undefined): boolean => {
  const fly = boxOf(content);
  return !!aim.from && !!fly && inTriangle(p, aimTriangle(aim.from, fly));
};

/** The window's pointer moves and keys (capture: before Zag's own listeners), and the row's pointer leaving it. */
function usePointerAndKeys(deps: OpenDeps, aim: Aim, h: { readonly open: Accessor<boolean>; readonly hold: () => void; readonly close: () => void; readonly stopHold: () => void }): void {
  const onMove = (e: PointerEvent): void => {
    if (e.pointerType !== 'mouse') return;
    const p = { x: e.clientX, y: e.clientY };
    aim.last = p;
    if (!aim.holding) return;
    if (onBox(p, deps.content())) {
      // arrived: Zag's own pointerenter keeps it open from here
      aim.holding = false;
      aim.from = undefined;
      h.stopHold();
    } else if (onBox(p, deps.row())) {
      aim.holding = false;
      h.stopHold();
      deps.reopen();
    } else if (aiming(aim, p, deps.content())) {
      h.hold();
      // on its way: the rows it crosses neither light nor open their own flyouts
      e.stopPropagation();
    } else h.close();
  };
  const onKey = (e: KeyboardEvent): void => {
    aim.keyAt = performance.now();
    if (e.key !== 'Escape' || e.isComposing || !h.open() || !deps.content()?.contains(document.activeElement)) return;
    // Zag's Escape in a submenu closes the whole menu (a document capture listener): this runs first, in the window's
    // capture, and closes only the flyout, its row lit again
    e.preventDefault();
    e.stopPropagation();
    deps.back();
  };
  const left = (e: PointerEvent): void => {
    if (e.pointerType === 'mouse') aim.from = { x: e.clientX, y: e.clientY };
  };
  window.addEventListener('pointermove', onMove, true);
  window.addEventListener('keydown', onKey, true);
  onMount(() => deps.row()?.addEventListener('pointerleave', left));
  onCleanup(() => {
    window.removeEventListener('pointermove', onMove, true);
    window.removeEventListener('keydown', onKey, true);
    deps.row()?.removeEventListener('pointerleave', left);
  });
}

/** A submenu's row and its flyout. */
export function Flyout(props: { readonly api: menu.Api; readonly entry: MenuSubmenu; readonly part: PartName; readonly fly: FlyContext }): JSX.Element {
  const id = createUniqueId();
  let row: HTMLElement | undefined;
  let content: HTMLElement | undefined;
  let send: (event: { readonly type: string }) => void = () => {};
  const state = useFlyoutOpen({
    row: () => row,
    content: () => content,
    reopen: () => send({ type: 'MENU_POINTERENTER' }),
    back: () => send({ type: 'ARROW_LEFT' }),
    // Zag lights the first item only when it opens the child itself; controlled, the kit does (→ / Enter)
    keyedOpen: () => {
      const first = firstValue(props.entry.items);
      if (first) queueMicrotask(() => capi().setHighlightedValue(first));
    },
  });
  const child = useMachine(menu.machine, () => ({
    id,
    open: state.open(),
    onOpenChange: (d: menu.OpenChangeDetails) => state.request(d.open),
    // a flyout's item is chosen on the pointer's release, before the click that ends the press: what it opens (a dialog)
    // would take that click as one outside it and close at once (QA r14 #4: Recipes › New). Chosen once the press ends.
    onSelect: (d: menu.SelectionDetails) => {
      const v = d.value;
      nextTask(() => props.fly.onSelect(v));
    },
    positioning: { overflowPadding: 8, flip: true, slide: true },
  }));
  send = (event) => child.send(event);
  const capi = createMemo(() => menu.connect(child, normalizeProps));
  // the machines start on mount: link them then
  onMount(() => {
    props.api.setChild(child);
    capi().setParent(props.fly.parent);
  });
  onCleanup(props.fly.adopt(state.close));
  const Entries = props.fly.Entries;
  return (
    <>
      <div
        {...props.api.getTriggerItemProps(capi())}
        ref={(el) => (row = el)}
        data-kit="menu-item"
        data-part={props.entry.part ?? sub(props.part, 'sub')}
        data-sub=""
        data-submenu={props.entry.value}
      >
        <Show when={props.entry.icon}>{(name) => <Icon name={name()} size="md" />}</Show>
        <span data-kit="menu-item-text">
          <span data-kit="menu-item-label" data-text="control">
            {props.entry.label}
          </span>
        </span>
        <Icon name="chevron" size="sm" tone="faint" />
      </div>
      <Portal>
        <div {...capi().getPositionerProps()} data-kit="positioner">
          <div {...capi().getContentProps()} ref={(el) => (content = el)} data-kit="menu" data-part={sub(props.part, 'flyout')} data-flyout="">
            <Entries api={capi()} items={props.entry.items} part={props.part} />
          </div>
        </div>
      </Portal>
    </>
  );
}
