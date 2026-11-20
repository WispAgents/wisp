// Tabs on Zag: arrow keys move between tabs, Home/End jump, one tab stop. Every panel stays
// mounted (hidden when not selected), so switching back keeps scroll and input.
import * as tabs from '@zag-js/tabs';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createEffect, createMemo, createSignal, createUniqueId, For, on, onCleanup, onMount, Show, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useGestureHost } from '../gestures/host.tsx';

export interface TabItem {
  readonly value: string;
  readonly label: string;
  readonly icon?: IconName;
  /** A count beside the label (tabular, so it doesn't wiggle). */
  readonly count?: number;
  readonly disabled?: boolean;
  /** its tooltip */
  readonly title?: string;
  /** `fit`: gives way to its icon before the row tightens (a tab another plugin pinned, a Watch page). Additive
   * (2026-11-19). */
  readonly minor?: boolean;
  /** The panel. Omit it to use the tabs as a segmented switch above content you render yourself. */
  readonly content?: JSX.Element;
}

export interface TabsProps extends KitProps {
  readonly items: readonly TabItem[];
  readonly value?: string;
  readonly defaultValue?: string;
  readonly onChange?: (value: string) => void;
  /** `line` (an underline, the default) or `pill` (a segmented control). */
  /** seg: v1 .pk-seg, a bordered strip of small choices, the picked one in the accent (Settings → Worktrees' filter) */
  readonly variant?: 'line' | 'pill' | 'seg';
  /** The tab list's accessible name. */
  readonly 'aria-label'?: string;
  /** Select on focus (automatic, the default) or on Enter/Space (manual). */
  readonly activation?: 'automatic' | 'manual';
  /** Stretch the tabs across the width (phone segments). */
  readonly fill?: boolean;
  /** seg only: v1 drew some strips as a `<fieldset class="pk-seg">` (Theme, Automations stop at; Worktrees' filter sets its own margin),
   * which keeps the browser's 2px side margin; its `<span class="pk-seg">` ones (Auto's Goal, Scout first) have none. */
  readonly fieldset?: boolean;
  /** Labels give way to icons only when the row has no room for them, by priority (design/layout.md: control text
   * collapses to its icon): the `minor` tabs at the end first, then the row tightens (`data-tight`), then the labelled
   * tabs drop their icon (`data-bare`), then the rest from the end, one at a time; the open tab keeps its label. They come back as room comes. For icon tabs in a bar that can
   * narrow (the task's side panel). The browser's look only: a phone's row scrolls instead. Additive (2026-11-19). */
  readonly fit?: boolean;
}

/** One look of the row, measured: each tab's width labelled (`widths`, in order), a tab's width as its icon alone, the
 * gap between tabs. */
export interface FitLook {
  readonly widths: readonly number[];
  readonly icon: number;
  readonly gap: number;
}

/** `fit`'s decision, DOM-free: how many tabs from the end (the open one aside) show their icon alone, and whether the
 * row is `tight`. The states in order, the first that fits `room` wins: v1's own padding with the `minor` tabs going to
 * their icon one at a time from the end; then the tight row; then the rest from the end, one at a time (owner
 * 2026-11-19: at 1280 the side panel keeps Changes, PR, Shell and Preview labelled, Thread and pinned tabs give way
 * first). `chrome` the row's own padding and border. Measured once in one pass, so nothing steps on a stale layout
 * (the first fit stepped while Solid was still mounting, saw the same overflow each time and turned every tab to an icon).
 * `bare`, when measured: the tight row with the labelled tabs' icons hidden, the step before any tab loses its words
 * (audit E14: "Changes 1" and "PR #2" with a Thread tab left Preview an icon at 1280). */
export function fitPlan(m: {
  readonly room: number;
  readonly normal: FitLook;
  readonly tight: FitLook;
  readonly bare?: FitLook;
  readonly chrome: number;
  readonly open: number;
  readonly minor: readonly boolean[];
}): FitResult {
  const others = m.normal.widths.map((_, i) => i).filter((i) => i !== m.open);
  const minors = others.filter((i) => m.minor[i] === true).length;
  const total = (look: FitLook, n: number): number => {
    const icons = new Set(others.slice(others.length - n));
    return m.chrome + look.gap * Math.max(0, look.widths.length - 1) + look.widths.reduce((sum, w, i) => sum + (icons.has(i) ? Math.min(look.icon, w) : w), 0);
  };
  for (let n = 0; n <= minors; n += 1) if (total(m.normal, n) <= m.room + 0.5) return { iconic: n, tight: false };
  if (total(m.tight, minors) <= m.room + 0.5) return { iconic: minors, tight: true };
  // then the labelled tabs drop their icons (a bare row), and only then does a tab lose its words
  const last = m.bare ?? m.tight;
  const bare = m.bare === undefined ? {} : { bare: true as const };
  for (let n = minors; n < others.length; n += 1) if (total(last, n) <= m.room + 0.5) return { iconic: n, tight: true, ...bare };
  // every other tab an icon: the room that's left goes back to the tabs whose label still fits, front first (audit E
  // #6: a pinned tab open at 1280 left Changes, PR, Shell and Preview as icons with Shell's label's room to spare, since
  // the widest, Changes, was always the last to keep its label)
  let used = total(last, others.length);
  const labelled: number[] = [];
  if (used <= m.room + 0.5) {
    for (const i of others) {
      const more = (last.widths[i] ?? 0) - Math.min(last.icon, last.widths[i] ?? 0);
      if (used + more <= m.room + 0.5) {
        used += more;
        labelled.push(i);
      }
    }
  }
  return { iconic: others.length - labelled.length, tight: true, ...(labelled.length > 0 && { ...bare, labelled }) };
}

/** `fitPlan`'s answer: the last `iconic` tabs (the open one aside) show their icon alone, or, when `labelled` is given,
 * every tab but the open one and those (indices in the row) does; `tight` the row's tighter padding. */
export interface FitResult {
  readonly iconic: number;
  readonly tight: boolean;
  /** the labelled tabs show their words alone (`data-bare`) */
  readonly bare?: boolean;
  readonly labelled?: readonly number[];
}

/** The phone apps (ADR-0065 step 6, design/mobile.md §A.7): every variant draws as M3 tabs on Android (`line` the primary
 * tabs, `pill` and `seg` the secondary ones) and as a segmented control on iOS, from the look's stylesheet; a change
 * ticks the picker haptic (§A.5). */
export function Tabs(props: TabsProps): JSX.Element {
  const id = createUniqueId();
  const look = usePlatform();
  const host = useGestureHost();
  const service = useMachine(tabs.machine, () => ({
    id,
    ...(props.value === undefined ? {} : { value: props.value }),
    defaultValue: props.defaultValue ?? props.items[0]?.value ?? null,
    onValueChange: (d: tabs.ValueChangeDetails) => {
      if (look() !== 'web') host.haptic('select');
      props.onChange?.(d.value);
    },
    activationMode: props.activation ?? 'automatic',
    loopFocus: true,
  }));
  const api = createMemo(() => tabs.connect(service, normalizeProps));
  const part = (): PartName => props.part ?? 'ui.tabs';
  let root: HTMLElement | undefined;
  useScrollEnds(() => root, () => look() !== 'web', () => api().value);
  const fits = (): boolean => props.fit === true && look() === 'web';
  const values = createMemo(() => props.items.map((i) => i.value), undefined, { equals: (a, b) => a.length === b.length && a.every((v, i) => v === b[i]) });
  const fit = useFit(() => root, fits, () => props.items, () => String(api().value ?? ''));
  // keyed by value, not by the item object: a caller that makes its items afresh on each change (a count, the open tab)
  // keeps its buttons, so the focus an arrow key moved stays on its tab (audit 2026-11-19: → worked once, then the
  // rebuilt button dropped the focus to the page)
  // `fit`: the last `iconic` tabs show their icon alone, the open one aside (you always read where you are)
  const iconicOf = (value: string): boolean => {
    const plan = fit();
    const n = plan.iconic;
    if (n === 0 || value === api().value) return false;
    if (plan.labelled) return !plan.labelled.includes(values().indexOf(value));
    const others = values().filter((v) => v !== api().value);
    return others.indexOf(value) >= others.length - n;
  };
  const byValue = createMemo(() => new Map(props.items.map((i) => [i.value, i])));
  onMount(() => keepWalkFocus(root));
  return (
    <div {...api().getRootProps()} ref={(el) => (root = el)} data-kit="tabs" data-part={part()} data-variant={props.variant ?? 'line'} data-fit={flag(fits())} data-tight={flag(fit().tight)} data-bare={flag(fit().bare)}>
      <div {...api().getListProps()} aria-label={props['aria-label']} data-kit="tab-list" data-part={sub(part(), 'list')} data-fill={flag(props.fill)} data-fieldset={flag(props.fieldset)}>
        <For each={values()}>
          {(value) => {
            const item = (): TabItem => byValue().get(value) ?? { value, label: value };
            return (
            <button {...api().getTriggerProps({ value, disabled: item().disabled })} data-kit="tab" data-part={sub(part(), 'tab')} data-value={value} data-icon={flag(item().icon !== undefined)} data-iconic={flag(iconicOf(value))}
              // an icon tab may show the icon alone (a narrow side panel hides its label): named and titled by it
              aria-label={item().icon ? (item().count !== undefined ? `${item().label} ${item().count}` : item().label) : undefined}
              title={item().title ?? (item().icon ? item().label : undefined)}
            >
              <Show when={item().icon}>{(name) => <Icon name={name()} size="md" />}</Show>
              <span data-kit="tab-label" data-text="control">
                {item().label}
              </span>
              <Show when={item().count !== undefined}>
                <span data-kit="tab-count" data-text="control">
                  {item().count}
                </span>
              </Show>
            </button>
            );
          }}
        </For>
      </div>
      <For each={props.items}>
        {(item) => (
          <Show when={item.content !== undefined}>
            <div {...api().getContentProps({ value: item.value })} data-kit="tab-panel" data-part={sub(part(), 'panel')}>
              {item.content}
            </div>
          </Show>
        )}
      </For>
    </div>
  );
}

/** An arrow key walks the tabs and the focus goes with the selection. In WebKit, with a pane that took 100–245 ms to show,
 * the focus fell to the page as the walk reached a pinned app's tab, and later Changes (audit H tasks NEW #2): the next
 * arrow had nothing to walk from. Within a moment of a walking key, a tab that loses the focus to nowhere (no new owner,
 * no press since) hands it to the selected tab. A press, or the focus going somewhere real (the Shell's keys), is left. */
export const WALK_KEEP_MS = 1500;
export function keepWalkFocus(root: HTMLElement | undefined, now: () => number = () => Date.now()): void {
  if (!root || typeof document === 'undefined') return;
  let walkedAt = -Infinity;
  const key = (e: KeyboardEvent): void => {
    if (/^(Arrow(Left|Right|Up|Down)|Home|End)$/.test(e.key) && (e.target as Element | null)?.closest?.("[data-kit='tab']")) walkedAt = now();
  };
  const press = (): void => void (walkedAt = -Infinity);
  const lost = (e: FocusEvent): void => {
    if (e.relatedTarget !== null || now() - walkedAt > WALK_KEEP_MS) return;
    if (!(e.target as Element | null)?.closest?.("[data-kit='tab']")) return;
    // a frame later: where the focus went is known (a field, the terminal) or it's the page
    const back = (): void => {
      frame = 0;
      const at = document.activeElement;
      if (at && at !== document.body && at.isConnected) return;
      // the keys left the page (Shift+Tab out to the browser): not ours to take back
      if (now() - walkedAt > WALK_KEEP_MS || !document.hasFocus()) return;
      root.querySelector<HTMLElement>("[data-kit='tab'][data-selected]")?.focus({ preventScroll: true });
    };
    if (typeof requestAnimationFrame === 'function') frame = requestAnimationFrame(back);
    else back();
  };
  let frame = 0;
  root.addEventListener('keydown', key, true);
  root.addEventListener('focusout', lost, true);
  document.addEventListener('pointerdown', press, true);
  onCleanup(() => {
    if (frame && typeof cancelAnimationFrame === 'function') cancelAnimationFrame(frame);
    root.removeEventListener('keydown', key, true);
    root.removeEventListener('focusout', lost, true);
    document.removeEventListener('pointerdown', press, true);
  });
}

/** `fit`: the row's plan (fitPlan), from one measuring pass: every tab labelled and as its icon, at v1's padding and
 * tight, read by setting the row's own attributes for a moment and putting them back, all before the browser paints
 * (no flash, no Solid update in between). Run when the tabs' box changes size (a ResizeObserver, never per frame), the
 * frame after the tabs change (a tab added, a label or count changed), and once the fonts are in. */
function useFit(root: () => HTMLElement | undefined, enabled: () => boolean, items: () => readonly TabItem[], open: () => string): () => FitResult {
  const [plan, setPlan] = createSignal<FitResult>({ iconic: 0, tight: false }, { equals: (a, b) => a.iconic === b.iconic && a.tight === b.tight && a.bare === b.bare && String(a.labelled ?? '') === String(b.labelled ?? '') });
  let check = (): void => {};
  onMount(() => {
    const el = root();
    const list = el?.querySelector<HTMLElement>("[data-kit='tab-list']");
    if (!el || !list || !enabled() || typeof ResizeObserver === 'undefined') return;
    check = (): void => {
      const tabs = [...list.querySelectorAll<HTMLElement>(":scope > [data-kit='tab']")];
      if (tabs.length === 0) return;
      const iconicWas = tabs.map((t) => t.hasAttribute('data-iconic'));
      const tightWas = el.hasAttribute('data-tight');
      const bareWas = el.hasAttribute('data-bare');
      // each tab at its own width while measured: the open tab doesn't give way to the row then (it does once drawn)
      el.setAttribute('data-measuring', '');
      const look = (tight: boolean, bare = false): FitLook => {
        el.toggleAttribute('data-tight', tight);
        el.toggleAttribute('data-bare', bare);
        for (const t of tabs) t.removeAttribute('data-iconic');
        const widths = tabs.map((t) => t.getBoundingClientRect().width);
        const gap = parseFloat(getComputedStyle(list).columnGap) || 0;
        const first = tabs[0] as HTMLElement;
        first.setAttribute('data-iconic', '');
        const icon = first.getBoundingClientRect().width;
        first.removeAttribute('data-iconic');
        return { widths, icon, gap };
      };
      const normal = look(false);
      const tight = look(true);
      const bare = look(true, true);
      el.toggleAttribute('data-tight', tightWas);
      el.toggleAttribute('data-bare', bareWas);
      tabs.forEach((t, i) => t.toggleAttribute('data-iconic', iconicWas[i] === true));
      el.removeAttribute('data-measuring');
      const cs = getComputedStyle(list);
      const px = (v: string): number => parseFloat(v) || 0;
      const chrome = px(cs.paddingLeft) + px(cs.paddingRight) + px(cs.borderLeftWidth) + px(cs.borderRightWidth);
      // a row pulled left by a negative margin (v1's -8px) has that much more room
      const room = el.clientWidth - Math.min(0, px(cs.marginLeft));
      const order = tabs.map((t) => t.getAttribute('data-value') ?? '');
      const byValue = new Map(items().map((i) => [i.value, i]));
      setPlan(fitPlan({ room, normal, tight, bare, chrome, open: order.indexOf(open()), minor: order.map((v) => byValue.get(v)?.minor === true) }));
    };
    const seen = new ResizeObserver(() => check());
    seen.observe(el);
    check();
    void document.fonts?.ready.then(() => check());
    onCleanup(() => seen.disconnect());
  });
  // what the tabs say changed (a tab added, a label or a count, the open one): plan again once it's drawn
  createEffect(
    on(
      () => `${open()}\u0002${items().map((i) => `${i.value}\u0000${i.label}\u0000${i.count ?? ''}\u0000${i.minor ? 1 : 0}`).join('\u0001')}`,
      () => {
        if (typeof requestAnimationFrame !== 'function') return;
        const frame = requestAnimationFrame(() => check());
        onCleanup(() => cancelAnimationFrame(frame));
      },
      { defer: true },
    ),
  );
  return plan;
}

/** A phone's tab row that runs out of room scrolls (QA r7 #5): the ends it can still scroll to are marked for the look
 * to fade, so a name is never cut hard at the edge, and the open tab is kept in view (as it opens, as the row is laid
 * out or resized). The browser's rows are left alone. */
function useScrollEnds(row: () => HTMLElement | undefined, phone: () => boolean, value: () => unknown): void {
  onMount(() => {
    const el = row();
    if (!el || !phone()) return;
    const mark = (): void => markEnds(el);
    el.addEventListener('scroll', mark, { passive: true });
    const seen = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(() => reveal(el));
    seen?.observe(el);
    const list = el.querySelector<HTMLElement>("[data-kit='tab-list']");
    if (list) seen?.observe(list);
    reveal(el);
    onCleanup(() => {
      el.removeEventListener('scroll', mark);
      seen?.disconnect();
    });
  });
  createEffect(on(value, () => phone() && ((el) => el && reveal(el))(row()), { defer: true }));
}

/** the ends of a scrolling tab row it can still scroll to: `data-scroll-start`, `data-scroll-end` */
function markEnds(el: HTMLElement): void {
  const max = el.scrollWidth - el.clientWidth;
  const at = el.scrollLeft;
  el.toggleAttribute('data-scroll-start', max > 1 && at > 1);
  el.toggleAttribute('data-scroll-end', max > 1 && at < max - 1);
}

/** the open tab scrolled into the row's view (only the row moves, never the page), then its ends marked */
function reveal(el: HTMLElement): void {
  const tab = el.querySelector<HTMLElement>("[data-kit='tab'][data-selected]");
  if (tab) {
    const box = el.getBoundingClientRect();
    const r = tab.getBoundingClientRect();
    if (r.right > box.right) el.scrollLeft += r.right - box.right;
    else if (r.left < box.left) el.scrollLeft -= box.left - r.left;
  }
  markEnds(el);
}
