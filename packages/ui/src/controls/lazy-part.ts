// A control's machine, loaded apart from the first page (ADR-0011: the entry stays under its budget). The overlays'
// Zag machines and floating-ui are most of a control's code, and the first screen opens none of them, so each control
// draws its trigger at once with plain props (its look, its roles, a press that answers within the frame: ADR-0017)
// and binds the machine when it's there. `preloadControls()` fetches them all once the first screen is up, and a
// trigger's first hover, focus or press fetches its own, so a press almost always finds its machine loaded; one that
// doesn't shows the control pressed and opens it as the machine lands.
import { createComponent, createMemo, createSignal, untrack, type Accessor, type Component, type JSX } from 'solid-js';

export interface LazyPart<M> {
  /** the module once it's loaded (a signal: a mounted control binds its machine then) */
  readonly ready: Accessor<M | undefined>;
  /** loads it (once; a failed fetch is tried again on the next call) */
  load(): Promise<M>;
}

const parts: LazyPart<unknown>[] = [];

export function lazyPart<M>(importer: () => Promise<M>): LazyPart<M> {
  const [ready, setReady] = createSignal<M>();
  let pending: Promise<M> | undefined;
  const part: LazyPart<M> = {
    ready,
    load: () =>
      (pending ??= importer().then(
        (m) => {
          setReady(() => m);
          return m;
        },
        (err: unknown) => {
          pending = undefined;
          throw err;
        },
      )),
  };
  parts.push(part as LazyPart<unknown>);
  return part;
}

/** A press that came before its machine: replayed on the same element once the machine is bound, so it does what it
 * would have (a click opens, ArrowDown opens on the first item, a right-click opens at the pointer). */
export type Replay =
  | { readonly el: HTMLElement; readonly type: 'click' }
  | { readonly el: HTMLElement; readonly type: 'keydown'; readonly key: string }
  | { readonly el: HTMLElement; readonly type: 'contextmenu'; readonly x: number; readonly y: number };

/** Once a machine is bound to its trigger (`id`): a trigger that already had focus tells it so (it took focus before
 * there was a machine to hear it, and the keyboard answers only a focused one), then the early press `r` is replayed
 * on its element, after this frame's props have reached it (the machine's ids and handlers). */
export function bound(id: unknown, r: Replay | undefined): void {
  queueMicrotask(() => {
    const el = typeof id === 'string' ? document.getElementById(id) : null;
    if (el && el === document.activeElement) el.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    if (!r?.el.isConnected) return;
    const init = { bubbles: true, cancelable: true };
    if (r.type === 'keydown') r.el.dispatchEvent(new KeyboardEvent('keydown', { ...init, key: r.key }));
    else if (r.type === 'contextmenu') r.el.dispatchEvent(new MouseEvent('contextmenu', { ...init, clientX: r.x, clientY: r.y }));
    else r.el.dispatchEvent(new MouseEvent('click', init));
  });
}

/** A trigger's first hover, focus or press: fetch its machine, ignoring a failed fetch (the press tries again). */
export const warm = (part: LazyPart<unknown>) => (): void => {
  part.load().catch(() => undefined);
};

/** Fetches every control's machine (the app calls it once its first screen is up and the page is idle). */
export function preloadControls(): Promise<void> {
  return Promise.all(parts.map((p) => p.load())).then(() => undefined);
}

/** A component that draws `part`'s once it's loaded (asking for it on first draw), and nothing until then: for one that
 * shows on a press (a popover's body), which preloadControls has usually fetched by then. */
export function lazyView<M, P extends object>(part: LazyPart<M>, pick: (m: M) => Component<P>): Component<P> {
  return (props) => {
    part.load().catch(() => undefined);
    return createMemo(() => {
      const m = part.ready();
      return m ? untrack(() => createComponent(pick(m), props)) : undefined;
    }) as unknown as JSX.Element;
  };
}
