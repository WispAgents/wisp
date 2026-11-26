// SettingsMasterDetail: a list on the left and the picked item's settings on the right, inside a settings pane
// (Settings → Agents: General, then one item per harness; harness audit r2). A search box above the list narrows it
// (the caller filters: by name and by the settings each item holds); ↑↓ on the list moves the pick, and the pick shows
// at once. Under 720px the list sits over the detail as one row of chips.
import { For, Show, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import '../css/settings-master.css';

export interface SettingsMasterItem {
  readonly id: string;
  readonly label: string;
  readonly logo?: string | undefined;
  readonly icon?: IconName | undefined;
  /** a quiet second line (its state in words) */
  readonly sub?: string | undefined;
  /** the state dot: ready, needs you, can't run, nothing to say */
  readonly dot?: 'good' | 'warn' | 'bad' | 'muted' | undefined;
}

export interface SettingsMasterDetailProps extends KitProps {
  /** the list's name, for a screen reader */
  readonly label: string;
  readonly items: readonly SettingsMasterItem[];
  readonly selected: string;
  readonly onSelect: (id: string) => void;
  readonly search: { readonly placeholder: string; readonly value: string; readonly onInput: (v: string) => void };
  /** the list when the search leaves nothing */
  readonly empty: string;
  /** the picked item's settings */
  readonly children: JSX.Element;
}

export function SettingsMasterDetail(props: SettingsMasterDetailProps): JSX.Element {
  const part = (): PartName => props.part ?? 'settings.master';
  let list: HTMLDivElement | undefined;
  const keys = (e: KeyboardEvent): void => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp' && e.key !== 'Home' && e.key !== 'End') return;
    const ids = props.items.map((i) => i.id);
    if (ids.length === 0) return;
    e.preventDefault();
    const at = ids.indexOf(props.selected);
    const next =
      e.key === 'Home' ? 0 : e.key === 'End' ? ids.length - 1 : at < 0 ? 0 : (at + (e.key === 'ArrowDown' ? 1 : -1) + ids.length) % ids.length;
    const id = ids[next];
    if (id === undefined) return;
    props.onSelect(id);
    queueMicrotask(() => list?.querySelector<HTMLButtonElement>(`[data-id="${CSS.escape(id)}"]`)?.focus());
  };
  return (
    <div id={props.id} data-kit="settings-master" data-part={part()}>
      <div data-kit="settings-master-side">
        <label data-kit="settings-master-search" data-part={sub(part(), 'search')}>
          <Icon name="search" />
          <input
            type="search"
            spellcheck={false}
            autocomplete="off"
            placeholder={props.search.placeholder}
            aria-label={props.search.placeholder}
            value={props.search.value}
            onInput={(e) => props.search.onInput(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown' && props.items.length > 0) {
                e.preventDefault();
                list?.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
              }
            }}
          />
        </label>
        <div ref={(el) => (list = el)} data-kit="settings-master-list" role="listbox" aria-label={props.label} aria-orientation="vertical" onKeyDown={keys}>
          <For each={props.items}>
            {(i) => (
              <button
                type="button"
                role="option"
                data-kit="settings-master-item"
                data-part={sub(part(), 'item')}
                data-id={i.id}
                aria-selected={i.id === props.selected ? 'true' : 'false'}
                data-on={flag(i.id === props.selected)}
                tabIndex={i.id === props.selected ? 0 : -1}
                onClick={() => props.onSelect(i.id)}
              >
                <span data-kit="settings-master-mark">
                  <Show when={i.logo} fallback={<Icon name={i.icon ?? 'agent'} />}>
                    {(src) => <img src={src()} alt="" />}
                  </Show>
                </span>
                <span data-kit="settings-master-text">
                  <b data-text="control">{i.label}</b>
                  <Show when={i.sub}>
                    <small>{i.sub}</small>
                  </Show>
                </span>
                <Show when={i.dot}>{(d) => <span data-kit="settings-master-dot" data-tone={d()} aria-hidden="true" />}</Show>
              </button>
            )}
          </For>
          <Show when={props.items.length === 0}>
            <p data-kit="settings-master-empty" data-part={sub(part(), 'empty')}>
              {props.empty}
            </p>
          </Show>
        </div>
      </div>
      <div data-kit="settings-master-detail" data-part={sub(part(), 'detail')}>
        {props.children}
      </div>
    </div>
  );
}
