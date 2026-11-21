// SettingsGroup: rows that belong together, in one bordered box (v1 .sx-group), with an optional
// head above it (v1 .sx-head: a title, one line, and buttons on the right).
// In the phone apps (ADR-0065 build step 4, design/mobile.md §A.2 Settings): Android's head is an M3 list
// subheader in the primary colour over rows with no card; iOS is an inset grouped section, its description
// under the box as the footer (HIG: help text in footers).
import { Show, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import '../css/settings.css';

export interface SettingsGroupProps extends KitProps, WithChildren {
  readonly title?: string;
  readonly description?: JSX.Element;
  readonly actions?: JSX.Element;
  /** the box's own part, when a group with a head names its box (v1 .sx-reach: the box, not the head) */
  readonly bodyPart?: PartName;
  /** 14px under it, whatever follows (v1 .mc-grid) */
  readonly spaceAfter?: boolean;
}

export function SettingsGroup(props: SettingsGroupProps): JSX.Element {
  const part = (): PartName => props.part ?? 'settings.group';
  const look = usePlatform();
  const footer = (): boolean => look() === 'ios';
  return (
    <section id={props.id} data-kit="settings-group" data-part={part()} data-head={flag(props.title !== undefined || props.actions !== undefined)} data-after={flag(props.spaceAfter)}>
      <Show when={props.title !== undefined || props.actions !== undefined}>
        <div data-kit="settings-group-head" data-part={sub(part(), 'head')}>
          <div data-kit="settings-group-titles">
            <Show when={props.title}>
              {(t) => (
                <h3 data-kit="settings-group-title" data-text="control">
                  {t()}
                </h3>
              )}
            </Show>
            <Show when={!footer() && props.description}>
              <p data-kit="settings-group-description">{props.description}</p>
            </Show>
          </div>
          <Show when={props.actions}>
            <div data-kit="settings-group-actions">{props.actions}</div>
          </Show>
        </div>
      </Show>
      <div data-kit="settings-group-body" data-part={props.bodyPart ?? sub(part(), 'body')}>
        {props.children}
      </div>
      <Show when={footer() && props.description}>
        <p data-kit="settings-group-footer" data-part={sub(part(), 'footer')}>
          {props.description}
        </p>
      </Show>
    </section>
  );
}
