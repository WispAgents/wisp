// Switch (v1's button role=switch) and Checkbox (on Zag: a real hidden input for forms, the right roles and keyboard).
// Checkbox draws at once on a plain input that already works; Zag's machine (./checkbox-machine.ts) loads apart from the
// first page (./lazy-part.ts) and binds to the same elements when it's there.
import { createSignal, createUniqueId, Show, type Accessor, type JSX } from 'solid-js';
import { lazyPart, warm } from './lazy-part.ts';
import type { CheckboxApi } from './checkbox-machine.ts';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';

interface ToggleBase extends KitProps {
  /** Visible label beside the control. Without one, pass `aria-label`. */
  readonly label?: string;
  readonly 'aria-label'?: string;
  readonly disabled?: boolean;
  readonly name?: string;
  readonly value?: string;
}

export interface SwitchProps extends ToggleBase {
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
}

/** An on/off setting that applies at once (Settings rows). v1's element: a `<button role="switch">` (Space and Enter
 * toggle it, it takes focus like any button), so a page reads and counts it as v1's did; a hidden input carries it in a
 * form when it has a `name`. */
export function Switch(props: SwitchProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.switch';
  const state = (): string => (props.checked ? 'checked' : 'unchecked');
  // the button is the track (v1 .switch: 34 × 20), its knob inside; a label sits beside it, outside the button
  const track = (): JSX.Element => (
    <button
      type="button"
      id={props.id}
      role="switch"
      aria-checked={props.checked}
      aria-label={props.label ? undefined : props['aria-label']}
      disabled={props.disabled === true}
      data-kit="switch"
      data-part={part()}
      data-state={state()}
      data-disabled={props.disabled ? '' : undefined}
      onClick={() => props.onChange(!props.checked)}
    >
      <Show when={props.name}>{(n) => <input type="hidden" name={n()} value={props.checked ? (props.value ?? 'on') : ''} />}</Show>
      <span data-kit="switch-control" data-part={sub(part(), 'control')} data-state={state()}>
        <span data-kit="switch-thumb" data-state={state()} />
      </span>
    </button>
  );
  return (
    <Show when={props.label} fallback={track()}>
      {(label) => (
        <label data-kit="switch-field">
          {track()}
          <span data-kit="toggle-label">{label()}</span>
        </label>
      )}
    </Show>
  );
}

export interface CheckboxProps extends ToggleBase {
  readonly checked: boolean | 'indeterminate';
  readonly onChange: (checked: boolean) => void;
  readonly invalid?: boolean;
}

const machine = lazyPart(() => import('./checkbox-machine.ts'));

export function Checkbox(props: CheckboxProps): JSX.Element {
  const id = props.id ?? createUniqueId();
  const [api, setApi] = createSignal<Accessor<CheckboxApi>>();
  const bound = (): CheckboxApi | undefined => api()?.();
  const part = (): PartName => props.part ?? 'ui.checkbox';
  const state = (): string => (props.checked === 'indeterminate' ? 'indeterminate' : props.checked ? 'checked' : 'unchecked');
  const look = (): Record<string, unknown> => ({ 'data-state': state(), 'data-disabled': props.disabled ? '' : undefined, 'data-invalid': props.invalid ? '' : undefined });
  const checked = (): boolean => bound()?.checked ?? props.checked === true;
  const indeterminate = (): boolean => bound()?.indeterminate ?? props.checked === 'indeterminate';
  return (
    <label {...(bound()?.getRootProps() ?? { ...look(), onPointerEnter: warm(machine), onFocusIn: warm(machine) })} data-kit="checkbox" data-part={part()}>
      <Show when={machine.ready()}>
        {(m) => {
          setApi(() => m().useCheckbox(props, id));
          return null;
        }}
      </Show>
      <input
        {...(bound()?.getHiddenInputProps() ?? {
          type: 'checkbox',
          id: `checkbox:${id}:input`,
          checked: props.checked === true,
          disabled: props.disabled === true,
          name: props.name,
          value: props.value ?? 'on',
          onChange: (e: Event) => props.onChange((e.currentTarget as HTMLInputElement).checked),
        })}
        aria-label={props.label ? undefined : props['aria-label']}
      />
      <span {...(bound()?.getControlProps() ?? { ...look(), 'aria-hidden': true })} data-kit="checkbox-control" data-part={sub(part(), 'control')}>
        <Show when={indeterminate()} fallback={<Show when={checked()}><Icon name="check" size="xs" /></Show>}>
          <Icon name="minus" size="xs" />
        </Show>
      </span>
      <Show when={props.label}>
        {(label) => (
          <span {...(bound()?.getLabelProps() ?? look())} data-kit="toggle-label">
            {label()}
          </span>
        )}
      </Show>
    </label>
  );
}
