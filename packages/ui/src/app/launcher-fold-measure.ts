// Measures the prompt box's toolbar for launcher-fold.ts (design/layout.md §2: collapsing is measured, not guessed):
// one ResizeObserver on the bar and each item on it, read once a frame (so a fold never re-enters the observer: Chrome's
// "ResizeObserver loop" and WebKit's page error, QA r1 #16), then the bar's form (its labels, or its icons) and how many
// of the tail's tools go into More. Each form's widths are remembered while the other shows, so the plan is stable at
// any width, in both engines alike (QA desktop r3 #3: WebKit's box was 10 px narrower and v1's 520 px rule took every
// label with room to spare).
import { createEffect, createMemo, createSignal, on, onCleanup, type Accessor } from 'solid-js';
import { barPlan, type BarForm, type LauncherTool } from './launcher-fold.ts';

/** the More button's width until it has been seen (an icon button, `--size-icon-btn`) */
const MORE = 30;

/** the bar's items other than the tail's shown tools (their widths and how many, for the gaps), and More's width; the
 *  shown tools' widths go into `widths` */
export function sumBar(bar: HTMLElement, shown: ReadonlySet<string>, widths: Map<string, number>, folded: boolean): { others: number; count: number; more: number } {
  let others = 0;
  let count = 0;
  let more = MORE;
  for (const el of bar.children) {
    if (!(el instanceof HTMLElement)) continue;
    // not drawn (display: none): no width and no gap; drawn at 0 px (the grow spacer): still a flex item, its gaps count
    if (el.getClientRects().length === 0) continue;
    // the grow spacer takes whatever is left: its gaps count, its width never does (else the bar always "fits" to the
    // pixel, and a pixel of rounding squeezed a wide bar to its icons)
    const w = el.dataset['kit'] === 'grow' ? 0 : el.offsetWidth;
    const a = el.dataset['a'];
    if (a !== undefined && shown.has(a)) widths.set(a, w);
    else if (a === 'more' && folded) more = w;
    else {
      others += w;
      count++;
    }
  }
  return { others, count, more };
}

export interface TailFold {
  readonly ref: (el: HTMLElement) => void;
  /** how many of the tail's last tools are in More */
  readonly folded: Accessor<number>;
  /** how far its labels have gone to their icons (launcher-fold.ts SQUEEZE): 0 none */
  readonly squeeze: Accessor<number>;
  /** even so it runs past its box: it scrolls, Send pinned over its end */
  readonly scroll: Accessor<boolean>;
}

/** A bar with no tail is measured too: its labels step to icons until it fits (UX 2026-11-18 #3's footer has a tail
 *  only while a feature is on). */
export function useTailFold(given: { readonly tail: () => readonly LauncherTool[]; readonly on: () => boolean }): TailFold {
  // read through memos the hook owns: the measure runs in a frame callback, outside any owner, and a host's prop getter
  // builds its tools (memos) on each read: there they were computations nobody owned (QA round 6 desktop #3, phone #9)
  const o = { tail: createMemo(given.tail), on: createMemo(given.on) };
  const [folded, setFolded] = createSignal(0);
  const [squeeze, setSqueeze] = createSignal(0);
  const [scroll, setScroll] = createSignal(false);
  const off = { ref: () => {}, folded, squeeze, scroll };
  if (typeof ResizeObserver === 'undefined' || typeof MutationObserver === 'undefined') return off;
  const forms: (BarForm | undefined)[] = [];
  let bar: HTMLElement | undefined;
  let frame = 0;
  const measure = (): void => {
    frame = 0;
    if (!bar?.isConnected) return;
    const ids = o.tail().map((t) => t.id);
    if (!o.on()) {
      if (folded() !== 0) setFolded(0);
      if (squeeze() !== 0) setSqueeze(0);
      return;
    }
    const form = squeeze();
    const k = Math.min(folded(), ids.length);
    const shown = new Set(ids.slice(0, ids.length - k));
    const widths = new Map(forms[form]?.tail ?? []);
    const { others, count, more } = sumBar(bar, shown, widths, k > 0);
    const cs = getComputedStyle(bar);
    const gap = Number.parseFloat(cs.columnGap) || 0;
    forms[form] = { others: others + gap * Math.max(0, count - 1), gap, tail: widths };
    // its content box: the bar's own padding is no room for its items (bar-labels.ts measures the same)
    const available = bar.clientWidth - (Number.parseFloat(cs.paddingLeft) || 0) - (Number.parseFloat(cs.paddingRight) || 0);
    const plan = barPlan({ available, ids, forms, more });
    setScroll(bar.scrollWidth > bar.clientWidth + 1);
    if (plan.level === squeeze() && plan.fold === folded()) return;
    if (plan.level !== squeeze()) setSqueeze(plan.level);
    if (plan.fold !== folded()) setFolded(plan.fold);
    // measured again next frame: a step that resizes nothing (Goal without its words) wakes no observer
    soon();
  };
  const soon = (): void => {
    if (frame === 0) frame = requestAnimationFrame(measure);
  };
  const ro = new ResizeObserver(soon);
  // an item drawn after a fold (More, a tool back from it) is measured too
  const mo = new MutationObserver((records) => {
    for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLElement) ro.observe(n);
    soon();
  });
  createEffect(on([() => o.tail().map((t) => t.id).join(' '), o.on], soon, { defer: true }));
  onCleanup(() => {
    ro.disconnect();
    mo.disconnect();
    if (frame !== 0) cancelAnimationFrame(frame);
  });
  return {
    ref: (el) => {
      bar = el;
      ro.observe(el);
      for (const kid of el.children) ro.observe(kid);
      mo.observe(el, { childList: true });
    },
    folded,
    squeeze,
    scroll,
  };
}
