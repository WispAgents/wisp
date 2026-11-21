// NavigationBar: the phone apps' destinations (design/mobile.md §A.1, §A.7): Tasks · Crew · Stuff · Computers.
// Android: the M3 navigation bar (80 dp, the 64 × 32 active indicator pill), a navigation rail (80 dp wide)
// from 600 dp. iOS: the tab bar (49 pt over the home indicator), the selected tab's glyph filled; a sidebar
// on iPad widths. The browser has no navigation bar (v1's look, ADR-0016): it draws nothing there.
//
// Selecting answers in the same frame: the item is marked current from the press (`selected` follows from
// `navigate()`, which never waits on data), and the pill's state layer shows the press itself.
import { onCleanup, Show, type JSX } from 'solid-js';
import { Key } from '@zag-js/solid';
import { flag, type KitProps } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { markRoot } from '../platform/root-mark.ts';

export interface NavigationItem {
  readonly id: string;
  /** The destination's name, under its icon (never wraps; ellipsis at 200 % text). */
  readonly label: string;
  readonly icon: IconName;
  /** The selected glyph (iOS fills it; M3 too where the set has one). Default: `icon`. */
  readonly iconSelected?: IconName;
  /** A count on the icon (the need-you count on Tasks); 0 or absent draws none. */
  readonly badge?: number;
  /** Not offered yet (the phone's Tasks before a computer is open and connected): drawn dimmed, a press does nothing.
   *  Additive. */
  readonly disabled?: boolean;
}

export interface NavigationBarProps extends KitProps {
  /** Required: what the destinations are ("Destinations"). */
  readonly 'aria-label': string;
  readonly items: readonly NavigationItem[];
  readonly selected: string;
  readonly onSelect: (id: string) => void;
  /** The badge's accessible text, given the count ("3 need you"). */
  readonly badgeLabel?: (n: number) => string;
}

export function NavigationBar(props: NavigationBarProps): JSX.Element {
  const look = usePlatform();
  return (
    <Show when={look() !== 'web'}>
      {/* a snackbar / the iOS capsule sits above it */}
      <nav ref={() => onCleanup(markRoot('data-nav-bar'))} id={props.id} aria-label={props['aria-label']} data-kit="nav-bar" data-part={props.part ?? 'ui.nav-bar'}>
        <Key each={props.items} by={(i) => i.id}>
          {(item) => {
            const on = (): boolean => item().id === props.selected;
            const badge = (): number => item().badge ?? 0;
            return (
              <button
                type="button"
                data-kit="nav-item"
                data-part={`${props.part ?? 'ui.nav-bar'}.item`}
                data-item={item().id}
                data-selected={flag(on())}
                data-disabled={flag(item().disabled === true)}
                disabled={item().disabled === true}
                aria-current={on() ? 'page' : undefined}
                onClick={() => item().disabled !== true && props.onSelect(item().id)}
              >
                <span data-kit="nav-pill">
                  <Icon name={on() ? (item().iconSelected ?? item().icon) : item().icon} />
                  <Show when={badge() > 0}>
                    <span data-kit="nav-badge" aria-label={props.badgeLabel?.(badge())}>
                      {badge() > 99 ? '99+' : String(badge())}
                    </span>
                  </Show>
                </span>
                <span data-kit="nav-label" data-text="control">
                  {item().label}
                </span>
              </button>
            );
          }}
        </Key>
      </nav>
    </Show>
  );
}
