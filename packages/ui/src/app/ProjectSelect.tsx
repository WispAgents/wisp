// The one project selector (owner, 2026-11-12; multi-pick 2026-11-13; everywhere a project is chosen, 2026-11-15): one
// control where a row of chips was. It reads like the chip that's on: the project's dot (several overlapping when
// several are picked), its name ("api + web"), how many need you, and a ▾ that says it opens a switcher. The sidebar's
// head, New task's place and the phone apps' Tasks bar all draw this one; what it opens (the ⌘P list) is the host's.
import { For, Show, type JSX } from 'solid-js';
import { flag, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';

export interface ProjectSelectProps {
  /** one name, "api + web", "4 projects", All projects or No folder */
  readonly label: string;
  /** the picked projects' colours: one dot, overlapping dots for several, none for the icon */
  readonly colors: readonly string[];
  /** with no colour: All projects' grid, No folder's page */
  readonly icon?: IconName;
  readonly need?: number | undefined;
  readonly needLabel?: string | undefined;
  readonly title?: string | undefined;
  readonly ariaLabel?: string | undefined;
  readonly part: PartName;
  /** where it sits: the sidebar's head, New task's place line, a phone app bar (the platform look) */
  readonly where?: 'head' | 'launcher' | 'bar';
  readonly onOpen: () => void;
}

export function ProjectSelect(props: ProjectSelectProps): JSX.Element {
  const several = (): boolean => props.colors.length > 1;
  return (
    <button
      type="button"
      data-kit="p-select"
      data-part={props.part}
      data-where={props.where}
      data-color={props.colors.length === 1 ? props.colors[0] : undefined}
      data-several={flag(several())}
      title={props.title}
      aria-label={props.ariaLabel}
      aria-haspopup="dialog"
      onClick={() => props.onOpen()}
    >
      <Show
        when={several()}
        fallback={
          <Show when={props.colors.length === 1} fallback={<Icon name={props.icon ?? 'feed'} />}>
            <i data-kit="p-dot" />
          </Show>
        }
      >
        <span data-kit="p-dots" data-part="topbar.project-dots">
          <For each={props.colors.slice(0, 3)}>{(c) => <i data-kit="p-dot" data-color={c} />}</For>
        </span>
      </Show>
      <span data-kit="p-name" data-text="control">
        {props.label}
      </span>
      <Show when={(props.need ?? 0) > 0}>
        <span data-kit="p-need" title={props.needLabel}>
          {props.need}
        </span>
      </Show>
      <Icon name="chevron" />
    </button>
  );
}
