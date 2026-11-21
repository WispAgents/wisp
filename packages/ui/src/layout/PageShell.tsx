// PageShell: a scrollable main view with a fixed header bar (Back button, icon, title/subtitle, trailing actions)
// and a centred, max-width scrolling body that works from 320 px to 1440 px.
import { Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { IconButton } from '../controls/Button.tsx';
import '../css/settings.css';

export interface PageShellBack {
  readonly label: string;
  readonly onBack: () => void;
}

export interface PageShellProps extends KitProps, WithChildren {
  readonly title: string;
  readonly subtitle?: string | undefined;
  readonly icon?: IconName | undefined;
  readonly back?: PageShellBack | undefined;
  readonly trailing?: JSX.Element | undefined;
  readonly bodyRef?: (el: HTMLDivElement) => void;
}

export function PageShell(props: PageShellProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.page';
  return (
    <section data-kit="page-shell" data-part={part()}>
      <header data-kit="page-shell-head" data-part={sub(part(), 'head')}>
        <Show when={props.back}>
          {(b) => <IconButton icon="back" variant="ghost" size="sm" label={b().label} onClick={() => b().onBack()} part={sub(part(), 'back')} />}
        </Show>
        <Show when={props.icon}>
          {(ic) => (
            <span data-kit="page-shell-icon" aria-hidden="true">
              <Icon name={ic()} size="sm" />
            </span>
          )}
        </Show>
        <div data-kit="page-shell-titles">
          <h1 data-kit="page-shell-title" data-part={sub(part(), 'title')}>
            {props.title}
          </h1>
          <Show when={props.subtitle}>
            {(s) => (
              <span data-kit="page-shell-sub" data-part={sub(part(), 'sub')}>
                {s()}
              </span>
            )}
          </Show>
        </div>
        <Show when={props.trailing}>
          <div data-kit="page-shell-actions" data-part={sub(part(), 'actions')}>
            {props.trailing}
          </div>
        </Show>
      </header>
      <div data-kit="page-shell-body" data-part={sub(part(), 'body')} ref={props.bodyRef}>
        <div data-kit="page-shell-inner" data-part={sub(part(), 'inner')}>
          {props.children}
        </div>
      </div>
    </section>
  );
}
