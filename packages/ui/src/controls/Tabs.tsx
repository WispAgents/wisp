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
   * collapses to its icon): the last tab first, then the one before, and come back as room comes; for icon tabs in a bar
   * that can narrow (the task's side panel). The browser's look only: a phone's row scrolls instead. Additive
   * (2026-11-19). */
  readonly fit?: boolean;
}

/** `fit`'s decision, DOM-free: `level` how many tabs from the end show their icon alone (`max` all of them), `room` the
 * width the tabs may take, `list` the row's box, `content` what its tabs need (scrollWidth), `needs[i]` what the row
 * needed at level i when it last gave way. One more tab goes to its icon while the row overflows; one comes back when
 * the room holds the row as it was a step back (no flapping between the two). */
export function fitLevel(m: { readonly level: number; readonly max: number; readonly room: number; readonly list: number; readonly content: number; readonly needs: readonly number[] }): {
  level: number;
  needs: number[];
} {
  if (m.content > m.list + 1) return m.level < m.max ? { level: m.level + 1, needs: [...m.needs.slice(0, m.level), m.content] } : { level: m.level, needs: [...m.needs] };
  const back = m.needs[m.level - 1];
  if (m.level > 0 && back !== undefined && m.room >= back) return { level: m.level - 1, needs: m.needs.slice(0, m.level - 1) };
  return { level: m.level, needs: [...m.needs] };
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
  const iconic = useFit(() => root, fits, () => props.items, () => String(api().value ?? ''));
  // keyed by value, not by the item object: a caller that makes its items afresh on each change (a count, the open tab)
  // keeps its buttons, so the focus an arrow key moved stays on its tab (audit 2026-11-19: → worked once, then the
  // rebuilt button dropped the focus to the page)
  const values = createMemo(() => props.items.map((i) => i.value), undefined, { equals: (a, b) => a.length === b.length && a.every((v, i) => v === b[i]) });
  // `fit`: the last `iconic()` tabs show their icon alone, the open one aside (you always read where you are)
  const iconicOf = (value: string): boolean => {
    const n = iconic();
    if (n === 0 || value === api().value) return false;
    const others = values().filter((v) => v !== api().value);
    return others.indexOf(value) >= others.length - n;
  };
  const byValue = createMemo(() => new Map(props.items.map((i) => [i.value, i])));
  return (
    <div {...api().getRootProps()} ref={(el) => (root = el)} data-kit="tabs" data-part={part()} data-variant={props.variant ?? 'line'} data-fit={flag(fits())}>
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

/** `fit`: how many tabs from the end show their icon alone. Measured when the tabs' box or the row changes size (a
 * ResizeObserver, never per frame), and the frame after the tabs change (a tab added, a label or count changed: the row's
 * box can stay the same width while what's in it grows, audit 2026-11-19 at 1280: Preview cut, pinned tabs clipped);
 * each step changes the row, so the next check follows until it fits. */
function useFit(root: () => HTMLElement | undefined, enabled: () => boolean, items: () => readonly TabItem[], open: () => string): () => number {
  const [level, setLevel] = createSignal(0);
  let needs: number[] = [];
  let check = (): void => {};
  onMount(() => {
    const el = root();
    const list = el?.querySelector<HTMLElement>("[data-kit='tab-list']");
    if (!el || !list || !enabled() || typeof ResizeObserver === 'undefined') return;
    // steps until the row settles: a step re-renders at once (a signal), so the next measure sees it; the row's box can
    // stay at its full width while what's in it shrinks, so no resize would come to take the next step
    check = (): void => {
      const max = Math.max(0, items().length - 1); // the open tab keeps its label
      for (let i = 0; i <= max + 1; i += 1) {
        const next = fitLevel({ level: Math.min(level(), max), max, room: el.clientWidth, list: list.clientWidth, content: list.scrollWidth, needs });
        needs = next.needs;
        if (next.level === level()) return;
        setLevel(next.level);
      }
    };
    const seen = new ResizeObserver(() => check());
    seen.observe(el);
    seen.observe(list);
    check();
    onCleanup(() => seen.disconnect());
  });
  // what the tabs say changed: the labelled row may need more (or less) than it had, so measure it again from labels
  createEffect(
    on(
      () => `${open()}\u0002${items().map((i) => `${i.value}\u0000${i.label}\u0000${i.count ?? ''}`).join('\u0001')}`,
      () => {
        if (typeof requestAnimationFrame !== 'function') return;
        const frame = requestAnimationFrame(() => {
          needs = [];
          setLevel(0); // measured afresh from the labelled row
          check();
        });
        onCleanup(() => cancelAnimationFrame(frame));
      },
      { defer: true },
    ),
  );
  return level;
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
