// VirtualList: any list over 200 rows (plan §5). Rows sit in normal flow between two spacers, are
// keyed by `key(item)` so a reorder moves the same DOM nodes (and their entry animations never
// replay), and are measured as they render. The list keeps what you are reading in place when a
// row above it changes height or new rows arrive; in `anchor="bottom"` (a transcript) it starts
// at the end and follows new rows while you are at the tail.
import { Key } from '@zag-js/solid';
import { batch, createEffect, createMemo, createSignal, on, onCleanup, onMount, type Accessor, type JSX, For } from 'solid-js';
import type { Size } from '@wisp/tokens';
import { sizeVar } from '@wisp/tokens';
import { flag, styleOf, sub, type KitProps, type Labelled, type PartName, type WithVars } from '../core/props.ts';
import { anchorAt, anchors, atTail, computeRange, offsets, restoreAnchor, type Anchor } from './virtual-math.ts';
import { PullRefresh } from './PullRefresh.tsx';
import { inObserverPass, markObserverPass, observeSoon } from '../core/observer-scope.ts';
import { bindLater } from '../gestures/binders-part.ts';
import { useGesture } from '../gestures/use.ts';
import { useT } from '../core/i18n.tsx';

export interface VirtualListProps<T> extends KitProps, WithVars, Labelled {
  readonly items: readonly T[];
  /** A stable identity per row: the same key keeps the same DOM node. */
  readonly key: (item: T) => string;
  readonly children: (item: Accessor<T>, index: Accessor<number>) => JSX.Element;
  /** Height in px of a row not measured yet. */
  readonly estimate?: number;
  /** Rows rendered beyond each edge of the viewport. */
  readonly overscan?: number;
  /** `bottom`: a transcript. Starts at the end and follows new rows while at the tail. */
  readonly anchor?: 'top' | 'bottom';
  /** The viewport height in px to plan for when the box has no height yet (first paint, tests). */
  readonly viewport?: number;
  /** A fixed height from the scale; otherwise it fills its parent (`grow`) or sizes by CSS. */
  readonly height?: Size;
  readonly grow?: boolean;
  /** Called when a bottom-anchored list arrives at or leaves its tail (for a *Latest* pill). */
  readonly onTailChange?: (atTail: boolean) => void;
  readonly ref?: (el: HTMLDivElement) => void;
  /** the row at this index sits a list gap below the one before it (data-gap: v1 .f-list's 2px between two task rows). Additive (2026-11-12). */
  readonly gapBefore?: (index: number) => boolean | 'section';
  /** the row of what's open beside the list (`data-current` on it; v1 painted it unlike the others). Additive. */
  readonly current?: (item: T) => boolean;
  /** runs of rows that belong together (a section: its heading and rows), each drawn as one box behind them, named
   *  `part`, so it can be measured and restyled as v1's section was (#secNeeds). Item indices, inclusive. Additive. */
  readonly groups?: readonly { readonly part: string; readonly from: number; readonly to: number }[];
  /** Pulling the list down (the phone apps only: M3 indicator / iOS refresh control, ADR-0065 step 4). The
   *  browser draws nothing for it. It sits over the rows, so it never moves what is measured. Additive. */
  readonly pull?: { readonly progress: number; readonly refreshing: boolean; readonly label: string } | undefined;
  /** Pull to refresh, the gesture (ADR-0065 step 7, design/mobile.md §A.4; the phone apps only): pulling the list
   *  down from its top past the line runs `onRefresh` (the list's reconnect: every computer now, skipping the
   *  backoff), and the list draws the pull itself. Its twin is *Reconnect* in the bar's me menu (and *Retry* on the
   *  offline bar). The browser binds nothing. Additive. */
  readonly refresh?: { readonly onRefresh: () => void; readonly refreshing: boolean; readonly label: string } | undefined;
}

interface Measured {
  /** Bumped when any height changes, so the offsets recompute. */
  readonly version: Accessor<number>;
  readonly get: (key: string) => number | undefined;
  readonly observe: (el: HTMLElement, key: string) => void;
  readonly unobserve: (el: HTMLElement) => void;
  /** The list's own box: while it isn't rendered, what its rows report is ignored. */
  readonly observeBox: (el: HTMLElement) => void;
}

/** Row heights by key, fed by one ResizeObserver per list. The same observer watches the list's box: a list taken
 *  off screen (display: none: a phone's task page in front of it) reports every row at 0 px, and recording that would
 *  re-plan the window and swap its rows on every tap, then rebuild them all on Back (F-phone-8). While the box isn't
 *  rendered the heights stay what they were; rows that changed meanwhile report again when it shows. */
function useMeasured(beforeChange: () => void, onBox: (el: HTMLElement) => void = () => {}): Measured {
  const heights = new Map<string, number>();
  const keys = new WeakMap<Element, string>();
  const box: { el: Element | undefined; hidden: boolean } = { el: undefined, hidden: false };
  const [version, setVersion] = createSignal(0);
  const record = (el: Element, h: number): boolean => {
    const key = keys.get(el);
    if (key === undefined || heights.get(key) === h) return false;
    heights.set(key, h);
    return true;
  };
  // the rows observed now (to set aside while a re-plan's scrollbar change goes by, below)
  const rows = new Set<Element>();
  let back: Element[] = [];
  // a shown row's width now (rows all take the list's width)
  const rowWidth = (): number | undefined => {
    for (const el of rows) if (el.isConnected) return (el as HTMLElement).offsetWidth;
    return undefined;
  };
  let gone = false;
  const ro =
    typeof ResizeObserver === 'undefined'
      ? undefined
      : new ResizeObserver((entries) => {
          markObserverPass();
          inPass = true;
          try {
            measure(entries);
          } finally {
            inPass = false;
          }
        });
  // rows a re-plan mounts during the observer's own pass: observed on the next frame, not in this pass (where the
  // browser skips their first size at the rows' depth anyway, and WebKit reports the loop: QA round 2 #16, the
  // Diagnostics event list growing as it was measured). The re-plan itself is in this pass, before paint.
  let inPass = false;
  let fresh: Element[] = [];
  const measure = (entries: readonly ResizeObserverEntry[]): void => {
    for (const e of entries) {
      if (e.target !== box.el) continue;
      // the list's own height, from the layout this pass follows (no forced layout: perf r1 #4)
      onBox(e.target as HTMLElement);
      const b = e.borderBoxSize[0];
      box.hidden = (b?.inlineSize ?? e.contentRect.width) === 0 && (b?.blockSize ?? e.contentRect.height) === 0;
    }
    if (box.hidden) return;
    beforeChange();
    let changed = false;
    for (const e of entries) if (e.target !== box.el) changed = record(e.target, e.borderBoxSize[0]?.blockSize ?? e.contentRect.height) || changed;
    if (!changed) return;
    // the re-plan is made here, before paint (a frame later, the rows above the reader and a streaming tail jumped)
    const wide = rowWidth();
    setVersion((v) => v + 1);
    // ...but the heights it set can take the list's scrollbar away (or bring it): with a scrollbar that takes room
    // (a desktop's classic one; a phone's overlays never do) every row is then a little wider, in this same pass,
    // and the browser skips those changes at the rows' depth: WebKit's "ResizeObserver loop completed with
    // undelivered notifications" (QA round 1 #16, round 2 #15). The rows step out of the observer until the next
    // frame, which measures them again; what shows is right meanwhile (they're in the flow), only the spacers
    // wait that frame for a width-only change.
    // (the rows' own width: the scrollbar may be the list's, or a scroller's it sits in, as in the sidebar)
    if (wide === undefined || rowWidth() === wide) return;
    if (!back.length)
      requestAnimationFrame(() => {
        const now = back;
        back = [];
        if (!gone) for (const el of now) if (el.isConnected && rows.has(el)) ro?.observe(el);
      });
    for (const el of rows) {
      ro?.unobserve(el);
      back.push(el);
    }
  };
  onCleanup(() => {
    gone = true;
    ro?.disconnect();
  });
  return {
    version,
    get: (key) => heights.get(key),
    observe: (el, key) => {
      keys.set(el, key);
      rows.add(el);
      // (or another kit observer's: a list mounted by what that one started, audit B #4)
      if (!inPass && !inObserverPass()) return void ro?.observe(el);
      if (!fresh.length)
        requestAnimationFrame(() => {
          const now = fresh;
          fresh = [];
          if (!gone) for (const r of now) if (r.isConnected && rows.has(r)) ro?.observe(r);
        });
      fresh.push(el);
    },
    unobserve: (el) => {
      rows.delete(el);
      ro?.unobserve(el);
    },
    observeBox: (el) => {
      box.el = el;
      // its border box: a scrollbar coming or going changes its content box only, which (shallower than the rows) the
      // same pass would skip too (#15). A list mounted during another observer's pass is measured from the next frame,
      // where its box and rows are delivered together (core/observer-scope.ts, audit B #4)
      observeSoon(() => ro?.observe(el, { box: 'border-box' }), () => !gone && el.isConnected);
    },
  };
}

interface Scroller {
  readonly attach: (el: HTMLDivElement) => void;
  readonly scrollTop: Accessor<number>;
  readonly viewportH: Accessor<number>;
  readonly onScroll: () => void;
  /** Note the row under the viewport's top before something changes heights (`top`: a scrollTop already read). */
  readonly remember: (top?: number) => void;
  /** the list's box as its ResizeObserver reported it: the viewport's height from that pass */
  readonly boxed: (el: HTMLElement) => void;
}

/** Scroll position, the tail rule and the anchor row, for one list. */
function useScroller<T>(props: VirtualListProps<T>, offs: Accessor<readonly number[]>, indexOf: (key: string) => number): Scroller {
  const box: { el: HTMLDivElement | undefined } = { el: undefined };
  const bottom = (): boolean => props.anchor === 'bottom';
  const [scrollTop, setScrollTop] = createSignal(0);
  const [viewportH, setViewportH] = createSignal(props.viewport ?? 600);
  const [following, setFollowing] = createSignal(bottom());
  // What the reader is looking at, by key: survives inserts above it as well as height changes.
  const anchor: { at: { key: string; delta: number } | undefined } = { at: undefined };

  const remember = (top?: number): void => {
    if (!box.el) return;
    const at = top ?? box.el.scrollTop;
    if (!anchors(at, bottom() ? 'bottom' : 'top')) return void (anchor.at = undefined);
    const a: Anchor = anchorAt(offs(), at);
    const item = props.items[a.index];
    anchor.at = item === undefined ? undefined : { key: props.key(item), delta: a.delta };
  };
  const scrollTo = (el: HTMLDivElement, y: number): void => {
    el.scrollTop = y;
    setScrollTop(el.scrollTop);
  };
  // Geometry is read once, before the batch renders the new rows: a read after them would force a reflow on every
  // scroll frame (F-phone-8: 60 ms a frame on a 4× slowed phone with 4,000 tasks).
  const onScroll = (): void => {
    const el = box.el;
    if (!el) return;
    const top = el.scrollTop;
    const view = el.clientHeight;
    const tail = bottom() ? atTail(top, view || viewportH(), el.scrollHeight) : undefined;
    batch(() => {
      setScrollTop(top);
      if (view > 0) setViewportH(view);
      if (tail === undefined || tail === following()) return;
      setFollowing(tail);
      props.onTailChange?.(tail);
    });
    remember(top);
  };

  // After the rows or their heights change: follow the tail, or put the anchor row back.
  createEffect(
    on(
      offs,
      (o) => {
        const el = box.el;
        if (!el) return;
        if (bottom() && following()) return scrollTo(el, el.scrollHeight);
        const a = anchor.at;
        // by the key → index map (built once per change of items): this runs after every measurement pass and scroll
        // frame, where a findIndex over 5,000 rows was the list's own top cost (perf audit 2026-11-18 #14)
        const index = a ? indexOf(a.key) : -1;
        if (!a || index < 0) return;
        const want = restoreAnchor(o, { index, delta: a.delta });
        if (Math.abs(el.scrollTop - want) > 0.5) scrollTo(el, want);
      },
      { defer: true },
    ),
  );

  onMount(() => {
    const el = box.el;
    if (!el) return;
    // a list from the top takes its height from the first layout (the list's ResizeObserver pass, before paint: `boxed`):
    // read here, mid render, it forced the whole page's first layout (perf r1 #4: 109 ms of the first screen at 4×). A
    // list from the bottom scrolls to its end now, as before.
    if (!bottom() && typeof ResizeObserver !== 'undefined') return;
    if (el.clientHeight > 0) setViewportH(el.clientHeight);
    if (bottom()) scrollTo(el, el.scrollHeight);
  });

  /** the list's box as its ResizeObserver saw it: its height now (layout is fresh in that pass) */
  const boxed = (el: HTMLElement): void => {
    if (el.clientHeight > 0 && el.clientHeight !== viewportH()) setViewportH(el.clientHeight);
  };
  return { attach: (el) => (box.el = el), scrollTop, viewportH, onScroll, remember, boxed };
}

export function VirtualList<T>(props: VirtualListProps<T>): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.virtual-list';
  // The observer calls back only after mount, by which time `scroller` exists.
  const measured = useMeasured(
    () => scroller.remember(),
    (el) => scroller.boxed(el),
  );
  // the rows' keys and where each sits, once per change of items: a measurement pass (every scroll frame that brings
  // rows in) then reads heights by key without calling `key` 5,000 times (perf audit 2026-11-18 #14)
  const keys = createMemo(() => props.items.map((item) => props.key(item)));
  const indexes = createMemo(() => new Map(keys().map((k, i) => [k, i] as const)));
  const offs = createMemo(() => {
    measured.version();
    const est = props.estimate ?? 36;
    return offsets(keys().map((k) => measured.get(k) ?? est));
  });
  const scroller = useScroller(props, offs, (key) => indexes().get(key) ?? -1);
  const range = createMemo(() => computeRange(offs(), scroller.scrollTop(), scroller.viewportH(), props.overscan ?? 6));
  const visible = createMemo(() => props.items.slice(range().start, range().end));
  const [pulled, setPulled] = createSignal(0);
  const t = useT();
  const pullGesture = useGesture(
    'list.pull',
    (el, host) => bindLater((b) => b.bindPull(el, { host, onRefresh: () => props.refresh?.onRefresh(), onProgress: setPulled, refreshing: () => props.refresh?.refreshing === true })),
    () => [{ label: t('gesture.reconnect'), run: () => props.refresh?.onRefresh() }],
  );
  const pullView = (): { readonly progress: number; readonly refreshing: boolean; readonly label: string } | undefined =>
    props.pull ?? (props.refresh ? { progress: pulled(), refreshing: props.refresh.refreshing, label: props.refresh.label } : undefined);

  const row = (item: Accessor<T>, i: Accessor<number>): JSX.Element => (
    <div
      role="listitem"
      data-kit="virtual-row"
      data-part={sub(part(), 'row')}
      data-gap={((g) => (g === 'section' ? 'section' : g ? '' : undefined))(props.gapBefore?.(range().start + i()))}
      data-current={props.current?.(item()) ? '' : undefined}
      ref={(el) => {
        measured.observe(el, props.key(item()));
        onCleanup(() => measured.unobserve(el));
      }}
    >
      {props.children(item, () => range().start + i())}
    </div>
  );

  return (
    <div
      ref={(el) => {
        scroller.attach(el);
        measured.observeBox(el);
        if (props.refresh) pullGesture(el);
        props.ref?.(el);
      }}
      id={props.id}
      role="list"
      aria-label={props['aria-label']}
      aria-labelledby={props['aria-labelledby']}
      data-kit="virtual-list"
      data-part={part()}
      data-anchor={props.anchor ?? 'top'}
      data-grow={flag(props.grow)}
      onScroll={() => scroller.onScroll()}
      style={styleOf({ '--k-vl-h': props.height === undefined ? undefined : sizeVar(props.height) }, props.vars)}
    >
      {pullView() ? <PullRefresh progress={pullView()?.progress ?? 0} refreshing={pullView()?.refreshing === true} label={pullView()?.label ?? ''} part={sub(part(), 'pull')} /> : null}
      <div data-kit="virtual-spacer" aria-hidden="true" style={{ height: `${range().padTop}px` }} />
      <Key each={visible()} by={(item: T) => props.key(item)}>
        {row}
      </Key>
      <div data-kit="virtual-spacer" aria-hidden="true" style={{ height: `${range().padBottom}px` }} />
      <For each={props.groups ?? []}>
        {(g) => (
          <div
            data-kit="virtual-group"
            data-part={g.part}
            aria-hidden="true"
            style={{ top: `${offs()[g.from] ?? 0}px`, height: `${Math.max(0, (offs()[g.to + 1] ?? 0) - (offs()[g.from] ?? 0))}px` }}
          />
        )}
      </For>
    </div>
  );
}

export interface EachProps<T> {
  readonly items: readonly T[];
  readonly key: (item: T) => string;
  readonly children: (item: Accessor<T>, index: Accessor<number>) => JSX.Element;
  readonly fallback?: JSX.Element;
}

/**
 * A keyed list without virtualization, for lists under 200 rows: renders no element of its own.
 * Rows are kept by key, so a reorder or an updated item never re-creates a node.
 */
export function Each<T>(props: EachProps<T>): JSX.Element {
  return (
    <Key each={props.items} by={(item: T) => props.key(item)} fallback={props.fallback}>
      {(item, i) => props.children(item, i)}
    </Key>
  );
}
