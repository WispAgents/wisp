// A button on the prompt box's toolbar (the launcher's and the reply box's, docs/ux/2026-11-18-desktop.md #3): an icon
// (and its words), its press or menu or panel; a feature that's on with its own clear is a chip with an ✕.
import { Show, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { ActIcon, WithAct } from './act.tsx';
import type { LauncherTool } from './launcher-fold.ts';

export function Tool(props: { readonly t: LauncherTool }): JSX.Element {
  // a feature that's on with its own clear: a chip, the tool's press and an ✕ (docs/ux/2026-11-18-desktop.md #3); the
  // chip is what the bar measures and folds (its data-a)
  return (
    <Show when={props.t.clear} fallback={<ToolButton t={props.t} />}>
      {(c) => (
        <span data-kit="l-chip" data-a={props.t.id} data-on="">
          <ToolButton t={props.t} inChip />
          <WithAct
            act={c().act}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="chip-x" data-part={`${props.t.part}.clear`} title={c().label} aria-label={c().label}>
                <Icon name="x" />
              </button>
            )}
          />
        </span>
      )}
    </Show>
  );
}

function ToolButton(props: { readonly t: LauncherTool; readonly inChip?: boolean }): JSX.Element {
  return (
    <WithAct
      act={props.t.disabled ? undefined : props.t.act}
      render={(trigger) => (
        <button
          type="button"
          {...trigger()}
          data-kit={props.t.text === undefined ? 'icon-btn' : 'tog-btn'}
          data-a={props.inChip ? undefined : props.t.id}
          data-part={props.t.part}
          data-on={flag(props.t.on)}
          title={props.t.title}
          aria-label={props.t.label}
          aria-pressed={props.t.on === undefined ? undefined : props.t.on}
          aria-disabled={props.t.disabled ? 'true' : undefined}
        >
          <ActIcon act={props.t.act}>
            <Icon name={props.t.icon} />
          </ActIcon>
          <Show when={props.t.text}>
            <span data-text="control">{props.t.label}</span>
          </Show>
        </button>
      )}
    />
  );
}
