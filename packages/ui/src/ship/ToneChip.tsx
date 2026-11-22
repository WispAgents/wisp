// ToneChip: v1's PR pill as a button (css/ship.css): a dot in its tone, a one-line label.
import { Show, type JSX } from 'solid-js';
import type { Tone } from '@wisp/tokens';
import { flag, type KitProps } from '../core/props.ts';
import '../css/ship.css';

export interface ToneChipProps extends KitProps {
  readonly label: string;
  /** Trailing non-shrinking text (e.g. "#1234") kept whole while a long label truncates before it. */
  readonly suffix?: string | undefined;
  readonly tone?: Tone;
  /** Show the status dot (default true); omit for calm utility chips like "+4 more". */
  readonly dot?: boolean | undefined;
  /** pulse the dot: checks are still running */
  readonly live?: boolean;
  readonly pressed?: boolean;
  readonly title?: string;
  readonly onClick?: () => void;
}

export function ToneChip(props: ToneChipProps): JSX.Element {
  return (
    <button
      type="button"
      id={props.id}
      title={props.title}
      data-kit="tone-chip"
      data-part={props.part ?? 'ui.tone-chip'}
      data-tone={props.tone ?? 'neutral'}
      data-pressed={flag(props.pressed)}
      aria-pressed={props.pressed === undefined ? undefined : props.pressed}
      onClick={(e) => {
        e.stopPropagation(); // a chip on a row opens its pane, not the row
        props.onClick?.();
      }}
    >
      <Show when={props.dot !== false}>
        <span data-kit="tone-chip-dot" data-live={flag(props.live)} />
      </Show>
      <span data-kit="tone-chip-label" data-text="control">{props.label}</span>
      <Show when={props.suffix}>
        {(s) => <span data-kit="tone-chip-suffix" data-text="control">{s()}</span>}
      </Show>
    </button>
  );
}
