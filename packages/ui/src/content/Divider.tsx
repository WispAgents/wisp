// Divider: a 1px neutral line. Separators use --line, a firmer one --line-2. With a `label`, the words sit in the
// middle of the line (a harness switch in a task's turns: "Switched from Claude Code (Opus) to Codex · context:
// summary"; harness audit r2), with an optional mark before them.
import { Show, type JSX } from 'solid-js';
import type { KitProps } from '../core/props.ts';

export interface DividerProps extends KitProps {
  readonly orientation?: 'horizontal' | 'vertical';
  readonly strength?: 'soft' | 'strong';
  /** words in the middle of a horizontal line (additive, 2026-11-25) */
  readonly label?: string | undefined;
  /** a logo before the words */
  readonly logo?: string | undefined;
}

export function Divider(props: DividerProps): JSX.Element {
  const orientation = (): 'horizontal' | 'vertical' => props.orientation ?? 'horizontal';
  return (
    <Show
      when={props.label}
      fallback={
        <hr
          id={props.id}
          aria-orientation={orientation()}
          data-kit="divider"
          data-part={props.part ?? 'ui.divider'}
          data-orientation={orientation()}
          data-strength={props.strength ?? 'soft'}
        />
      }
    >
      {(label) => (
        <div id={props.id} role="separator" aria-label={label()} data-kit="divider-label" data-part={props.part ?? 'ui.divider'} data-strength={props.strength ?? 'soft'}>
          <span data-kit="divider-label-text">
            <Show when={props.logo}>{(src) => <img src={src()} alt="" />}</Show>
            {label()}
          </span>
        </div>
      )}
    </Show>
  );
}
