// A pushed page's status pills (docs/ux/2026-11-18-phone.md #3), under its top app bar: what the task holds, each a
// tap to where it is (Changes +n −n, the PR) instead of a menu and a submenu away. Only the facts there are: no pill
// for an empty view. One line that scrolls sideways when it runs out of room (it never wraps, ADR-0012). The phone
// looks draw it (M3 assist chips on Android, capsule buttons on iOS); the browser's head never does.
import { For, Show, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import type { PartName } from '../core/props.ts';

export interface PageStatusItem {
  readonly id: string;
  readonly label: string;
  readonly icon?: IconName;
  /** its pane is open now */
  readonly on?: boolean;
  readonly onSelect: () => void;
}

export function PageStatus(props: { readonly items: readonly PageStatusItem[]; readonly label: string; readonly part?: PartName }): JSX.Element {
  return (
    <Show when={props.items.length > 0}>
      <div data-kit="page-status" data-part={props.part ?? 'ui.page-status'} role="toolbar" aria-label={props.label}>
        <For each={props.items}>
          {(it) => (
            <button
              type="button"
              data-kit="page-pill"
              data-part={`${props.part ?? 'ui.page-status'}.${it.id}`}
              aria-pressed={it.on === true}
              onClick={() => it.onSelect()}
            >
              <Show when={it.icon}>{(i) => <Icon name={i()} size="sm" />}</Show>
              <span data-text="control">{it.label}</span>
            </button>
          )}
        </For>
      </div>
    </Show>
  );
}
