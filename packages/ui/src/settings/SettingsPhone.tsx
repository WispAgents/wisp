// Settings in the phone apps (ADR-0065 step 14, design/mobile.md §A.2 Settings): SettingsFrame's phone half, the same
// props drawn as the platform's pushed pages. The list of panes is a page under the platform's bar (Android's M3 medium
// top app bar with Back; iOS's navigation bar, Back titled with the screen under it): Android an M3 settings list (each
// group's header in the primary colour, one-line items with their icon, no cards), iOS an inset grouped table (rows
// with a disclosure chevron). A pane is pushed over it: its own bar, Back to Settings (onUp), then the pane as it is,
// whose SettingsGroup / rows / switches / selects already draw the platform's. Loaded apart (settings-phone-load.ts)
// with the look: the browser never fetches it.
import { For, Show, createEffect, on, type JSX } from 'solid-js';
import { sub, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { ListRow } from '../composites/ListRow.tsx';
import { TopAppBar } from '../nav/TopAppBar.tsx';
import { SettingsGroup } from './SettingsGroup.tsx';
import type { SettingsFrameProps } from './SettingsFrame.tsx';
import '../css/settings.css';
import '../css/phone-pages.css';

export function SettingsPhone(props: SettingsFrameProps): JSX.Element {
  const part = (): PartName => props.part ?? 'settings.frame';
  const refs: { body?: HTMLDivElement } = {};
  // a pane opens at its top, and Back brings the list back at its top (one scroll box for both)
  createEffect(
    on(
      () => `${props.view}:${props.current}`,
      () => {
        if (refs.body) refs.body.scrollTop = 0;
      },
      { defer: true },
    ),
  );
  const close = (): void => props.onClose?.();
  return (
    <div id={props.id} data-kit="phone-page" data-page="settings" data-part={props.rootPart ?? part()} data-view={props.view} onKeyDown={(e) => props.onKeyDown?.(e)}>
      <Show
        when={props.view === 'pane'}
        fallback={
          <TopAppBar kind="page" part={sub(part(), 'bar')} title={props.title} back={{ label: props.backLabel ?? props.closeLabel, onBack: close }} />
        }
      >
        <TopAppBar kind="page" part={sub(part(), 'pane-bar')} title={props.paneTitle} back={{ label: props.title, onBack: () => props.onUp() }} />
      </Show>
      <div data-kit="phone-page-body" data-part={sub(part(), 'body')} ref={(el) => (refs.body = el)}>
        <Show
          when={props.view === 'pane'}
          fallback={
            <>
              <Show when={props.search}>
                <div data-kit="settings-nav-search">{props.search}</div>
              </Show>
              {/* focus lands here when Settings opens (Overlays' initial focus), never in a field that pops the keyboard */}
              <nav data-kit="phone-settings-list" data-part={sub(part(), 'nav')} aria-label={props.title} tabindex="-1" data-autofocus="">
                <For each={props.groups}>
                  {(g) => (
                    <SettingsGroup {...(g.label !== undefined && { title: g.label })} part={sub(part(), 'group')}>
                      <For each={g.items}>
                        {(item) => (
                          <ListRow
                            part={sub(part(), 'item')}
                            title={item.label}
                            push
                            lead={<Icon name={item.icon} size="md" />}
                            onOpen={() => props.onPick(item.id)}
                            {...(item.alert && { pills: <span data-kit="settings-nav-alert" data-tone={item.alert} role="img" aria-label={item.alertLabel} title={item.alertLabel} /> })}
                          />
                        )}
                      </For>
                    </SettingsGroup>
                  )}
                </For>
              </nav>
            </>
          }
        >
          <div data-kit="phone-settings-pane" data-pane={props.current} tabindex="-1" data-autofocus="" aria-label={props.paneTitle}>
            <Show when={props.lede}>
              <p data-kit="settings-lede">{props.lede}</p>
            </Show>
            {props.children}
          </div>
        </Show>
      </div>
    </div>
  );
}

export default SettingsPhone;
