// LabelDivider: a Divider with words in the middle of its line (a harness switch in a task's turns: "Switched from
// Claude Code (Opus) to Codex · context: summary"; harness audit r2), with an optional logo before them. Its own file
// and sheet, so only the lazy code that draws one carries them, not the first screen (ADR-0011).
import { Show, type JSX } from 'solid-js';
import type { KitProps } from '../core/props.ts';
import '../css/label-divider.css';

export interface LabelDividerProps extends KitProps {
  /** the words in the middle of the line */
  readonly label: string;
  /** a logo before the words */
  readonly logo?: string | undefined;
  readonly strength?: 'soft' | 'strong';
}

export function LabelDivider(props: LabelDividerProps): JSX.Element {
  return (
    <div id={props.id} role="separator" aria-label={props.label} data-kit="divider-label" data-part={props.part ?? 'ui.divider'} data-strength={props.strength ?? 'soft'}>
      <span data-kit="divider-label-text">
        <Show when={props.logo}>{(src) => <img src={src()} alt="" />}</Show>
        {props.label}
      </span>
    </div>
  );
}
