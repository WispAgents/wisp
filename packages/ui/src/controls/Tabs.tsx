// Tabs on Zag: arrow keys move between tabs, Home/End jump, one tab stop. Every panel stays
// mounted (hidden when not selected), so switching back keeps scroll and input.
import * as tabs from '@zag-js/tabs';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createEffect, createMemo, createUniqueId, For, on, onCleanup, onMount, Show, type JSX } from 'solid-js';
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
  return (
    <div {...api().getRootProps()} ref={(el) => (root = el)} data-kit="tabs" data-part={part()} data-variant={props.variant ?? 'line'}>
      <div {...api().getListProps()} aria-label={props['aria-label']} data-kit="tab-list" data-part={sub(part(), 'list')} data-fill={flag(props.fill)} data-fieldset={flag(props.fieldset)}>
        <For each={props.items}>
          {(item) => (
            <button {...api().getTriggerProps({ value: item.value, disabled: item.disabled })} data-kit="tab" data-part={sub(part(), 'tab')} data-value={item.value} data-icon={flag(item.icon !== undefined)}
              // an icon tab may show the icon alone (a narrow side panel hides its label): named and titled by it
              aria-label={item.icon ? (item.count !== undefined ? `${item.label} ${item.count}` : item.label) : undefined}
              title={item.title ?? (item.icon ? item.label : undefined)}
            >
              <Show when={item.icon}>{(name) => <Icon name={name()} size="md" />}</Show>
              <span data-kit="tab-label" data-text="control">
                {item.label}
              </span>
              <Show when={item.count !== undefined}>
                <span data-kit="tab-count" data-text="control">
                  {item.count}
                </span>
              </Show>
            </button>
          )}
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
