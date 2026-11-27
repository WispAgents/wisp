// The overlap note as v1 draws it (ship.css .overlap-strip): another task touches the same files.
import { Show, type JSX } from 'solid-js';
import { WithAct, type Act } from './act.tsx';
import { Icon } from '../content/Icon.tsx';

export interface OverlapNoteProps {
  readonly head: string;
  readonly files: string;
  readonly more?: string;
  readonly moreTitle: string;
  readonly open: string;
  readonly openTitle: string;
  readonly onOpen?: () => void;
  readonly moreAct?: Act;
  /** v1 #787 .ov-x: the note's ×, last on the row (the host keeps it closed until something changes) */
  readonly dismiss?: { readonly label: string; readonly title: string; readonly onDismiss: () => void };
}

/** v1 .overlap-strip: the first task changing the same files, above the reply box. */
export function OverlapNote(props: OverlapNoteProps): JSX.Element {
  return (
    <div data-kit="overlap-strip" data-part="card.overlapNote">
      <div data-kit="ov-row">
        <Icon name="merge" />
        <span data-kit="ov-text">
          <b>{props.head}</b> <span>{props.files}</span>
        </span>
        <Show when={props.more}>
          <WithAct
            act={props.moreAct}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="ov-more" title={props.moreTitle}>
                {props.more}
              </button>
            )}
          />
        </Show>
        <button type="button" data-kit="ghost-xs" data-compact="" title={props.openTitle} onClick={() => props.onOpen?.()}>
          {props.open}
        </button>
        <Show when={props.dismiss}>
          {(d) => (
            <button type="button" data-kit="ov-x" data-part="card.overlapClose" aria-label={d().label} title={d().title} onClick={() => d().onDismiss()}>
              <Icon name="x" />
            </button>
          )}
        </Show>
      </div>
    </div>
  );
}
