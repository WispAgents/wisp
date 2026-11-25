// SettingsSplit: a list and the one picked, inside a Settings pane (Settings → Agents: every harness, then the picked
// one's sections). A filter box over the list; each item its mark, its name, a state dot and, when there's one, a ring
// of how full it is. Up, Down, Home and End walk the list (the pick follows, as in the Settings list itself); Enter or
// Right goes on into the detail. Narrower than 640px of the pane's own width it shows the list, and a pick pushes the
// detail over it with Back, as the phone's Settings list does.
import { For, Show, createEffect, on, type JSX } from 'solid-js';
import type { Tone } from '@wisp/tokens';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { IconButton } from '../controls/Button.tsx';
import { Meter } from './Meter.tsx';
import { listStep } from './split-keys.ts';
import '../css/settings.css';

export interface SettingsSplitItem {
  readonly id: string;
  readonly label: string;
  readonly logo?: string | undefined;
  readonly icon?: IconName | undefined;
  /** the state dot: its tone and what it means (its tooltip and what a reader hears) */
  readonly dot?: { readonly tone: Tone; readonly label: string } | undefined;
  /** how full, 0..1: a ring after the name; none: no ring */
  readonly ring?: { readonly value: number; readonly label: string; readonly tone?: Tone | undefined } | undefined;
}

export interface SettingsSplitProps extends KitProps {
  /** the list's name, for readers ("Harnesses") */
  readonly label: string;
  /** above the list: a filter field */
  readonly filter?: JSX.Element;
  readonly items: readonly SettingsSplitItem[];
  readonly current: string;
  readonly onPick: (id: string) => void;
  /** narrow: the list, or the picked one over it */
  readonly view: 'list' | 'detail';
  /** narrow: Back to the list */
  readonly onBack: () => void;
  readonly backLabel: string;
  /** the picked one's name, over the detail when narrow */
  readonly detailTitle: string;
  /** what shows when the filter leaves nothing */
  readonly empty?: JSX.Element;
  readonly children: JSX.Element;
}

// size-ok: the list and its detail in one box, so the keys and the narrow push share one state
export function SettingsSplit(props: SettingsSplitProps): JSX.Element {
  const part = (): PartName => props.part ?? 'settings.split';
  let list: HTMLDivElement | undefined;
  let detail: HTMLDivElement | undefined;
  const focusItem = (id: string): void => list?.querySelector<HTMLElement>(`[data-item="${CSS.escape(id)}"]`)?.focus();
  const onKey = (e: KeyboardEvent): void => {
    const step = listStep(
      e.key,
      props.items.map((i) => i.id),
      props.current,
    );
    if (!step) return;
    e.preventDefault();
    if (step.pick !== undefined) {
      props.onPick(step.pick);
      focusItem(step.pick);
    }
    if (step.detail) detail?.focus();
  };
  // a new pick starts the detail at its top
  createEffect(
    on(
      () => props.current,
      () => {
        if (detail) detail.scrollTop = 0;
      },
      { defer: true },
    ),
  );
  return (
    <div id={props.id} data-kit="settings-split" data-part={part()} data-view={props.view}>
      <div data-kit="settings-split-list" data-part={sub(part(), 'list')}>
        <Show when={props.filter}>
          <div data-kit="settings-split-filter">{props.filter}</div>
        </Show>
        <div ref={list} data-kit="settings-split-items" role="tablist" aria-orientation="vertical" aria-label={props.label} onKeyDown={onKey}>
          <For each={props.items} fallback={props.empty}>
            {(item) => (
              <button
                type="button"
                data-kit="settings-split-item"
                data-part={sub(part(), 'item')}
                data-item={item.id}
                role="tab"
                aria-selected={props.current === item.id}
                tabindex={props.current === item.id || (!props.items.some((i) => i.id === props.current) && props.items[0]?.id === item.id) ? 0 : -1}
                data-current={flag(props.current === item.id)}
                onClick={() => props.onPick(item.id)}
              >
                <Show when={item.logo} fallback={<Icon name={item.icon ?? 'agent'} size="md" />}>
                  {(src) => <img data-kit="settings-split-logo" src={src()} alt="" />}
                </Show>
                <span data-kit="settings-split-label" data-text="control" title={item.label}>
                  {item.label}
                </span>
                <Show when={item.ring}>{(r) => <Meter shape="ring" value={r().value} label={r().label} tone={r().tone ?? 'accent'} part={sub(part(), 'ring')} />}</Show>
                <Show when={item.dot}>{(d) => <span data-kit="settings-split-dot" data-tone={d().tone} role="img" aria-label={d().label} title={d().label} />}</Show>
                <span data-kit="settings-split-chevron">
                  <Icon name="chevron" size="sm" />
                </span>
              </button>
            )}
          </For>
        </div>
      </div>
      <div ref={detail} data-kit="settings-split-detail" data-part={sub(part(), 'detail')} tabindex="-1" role="tabpanel" aria-label={props.detailTitle}>
        <div data-kit="settings-split-head">
          <IconButton icon="back" label={props.backLabel} onClick={() => props.onBack()} />
          <span data-kit="settings-split-title" data-text="control">
            {props.detailTitle}
          </span>
        </div>
        {props.children}
      </div>
    </div>
  );
}
