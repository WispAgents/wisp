// Measures the prompt box's toolbar for launcher-fold.ts (design/layout.md §2: collapsing is measured, not guessed):
// one ResizeObserver on the bar and each item on it, read once a frame (so a fold never re-enters the observer: Chrome's
// "ResizeObserver loop" and WebKit's page error, QA r1 #16), then the bar's form (its labels, or its icons) and how many
// of the tail's tools go into More. Each form's widths are remembered while the other shows, so the plan is stable at
// any width, in both engines alike (QA desktop r3 #3: WebKit's box was 10 px narrower and v1's 520 px rule took every
// label with room to spare).
import { createEffect, createSignal, on, onCleanup, type Accessor } from 'solid-js';
import { barPlan, type BarForm, type LauncherTool } from './launcher-fold.ts';

/** the More button's width until it has been seen (an icon button, `--size-icon-btn`) */
const MORE = 30;

/** the bar's items other than the tail's shown tools (their widths and how many, for the gaps), and More's width; the
 *  shown tools' widths go into `widths` */
function sumBar(bar: HTMLElement, shown: ReadonlySet<string>, widths: Map<string, number>, folded: boolean): { others: number; count: number; more: number } {
  let others = 0;
  let count = 0;
  let more = MORE;
  for (const el of bar.children) {
    if (!(el instanceof HTMLElement)) continue;
    // not drawn (display: none): no width and no gap; drawn at 0 px (the grow spacer): still a flex item, its gaps count
    if (el.getClientRects().length === 0) continue;
    const w = el.offsetWidth;
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

export function useTailFold(o: { readonly tail: () => readonly LauncherTool[]; readonly on: () => boolean }): TailFold {
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
    if (!o.on() || ids.length === 0) {
      if (folded() !== 0) setFolded(0);
      if (squeeze() !== 0) setSqueeze(0);
      return;
    }
    const form = squeeze();
    const k = Math.min(folded(), ids.length);
    const shown = new Set(ids.slice(0, ids.length - k));
    const widths = new Map(forms[form]?.tail ?? []);
    const { others, count, more } = sumBar(bar, shown, widths, k > 0);
    const gap = Number.parseFloat(getComputedStyle(bar).columnGap) || 0;
    forms[form] = { others: others + gap * Math.max(0, count - 1), gap, tail: widths };
    const plan = barPlan({ available: bar.clientWidth, ids, forms, more });
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
