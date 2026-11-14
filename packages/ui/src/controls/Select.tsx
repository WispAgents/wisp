// Select on Zag: a button the same box as an input, its options in a menu-like list (v1's
// .kit-sel). Typeahead, arrow keys, Home/End; a hidden native select carries it in forms.
// The button draws at once; the machine (./select-machine.tsx, with floating-ui) loads apart from the first page
// (./lazy-part.ts) and binds to it. A press before it's there shows the button pressed and opens the list as it lands.
import { createSignal, createUniqueId, For, Show, untrack, type Accessor, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type Labelled, type PartName } from '../core/props.ts';
import { lazyPart, warm, type Replay } from './lazy-part.ts';
import type { SelectApi } from './select-machine.tsx';
import { Icon } from '../content/Icon.tsx';
import { useField } from './Field.tsx';
import type { FieldSize } from './Input.tsx';

export interface SelectOption {
  readonly value: string;
  readonly label: string;
  readonly disabled?: boolean;
  /** A heading drawn over the first option of each run with the same group (the "When" picker's sources by plugin). */
  readonly group?: string;
}

export interface SelectProps extends KitProps, Labelled {
  readonly items: readonly SelectOption[];
  readonly value: string | undefined;
  readonly onChange: (value: string) => void;
  /** Shown while nothing is picked. */
  readonly placeholder?: string;
  readonly size?: FieldSize;
  readonly disabled?: boolean;
  readonly invalid?: boolean;
  readonly name?: string;
  /** Code-ish values. */
  readonly mono?: boolean;
}

const machine = lazyPart(() => import('./select-machine.tsx'));

type Bind = Record<string, unknown>;

export function Select(props: SelectProps): JSX.Element {
  const field = useField();
  const id = createUniqueId();
  const [api, setApi] = createSignal<Accessor<SelectApi>>();
  const [pending, setPending] = createSignal<Replay>();
  const bound = (): SelectApi | undefined => api()?.();
  const part = (): PartName => props.part ?? 'ui.select';
  const invalid = (): boolean => Boolean(props.invalid ?? field()?.invalid);
  const triggerId = (): string | undefined => field()?.id ?? props.id;
  const chosen = (): SelectOption | undefined => props.items.find((i) => i.value === props.value);
  const press = (r: Replay): void => {
    setPending(r);
    machine.load().catch(() => setPending(undefined));
  };
  const plainTrigger = (): Bind => ({
    type: 'button',
    id: triggerId() ?? `select:${id}:trigger`,
    role: 'combobox',
    'aria-haspopup': 'listbox',
    'aria-expanded': pending() !== undefined,
    'data-state': pending() ? 'open' : 'closed',
    disabled: props.disabled === true,
    'data-invalid': flag(invalid()),
    onPointerEnter: warm(machine),
    onFocus: warm(machine),
    onPointerDown: warm(machine),
    onClick: (e: MouseEvent) => press({ el: e.currentTarget as HTMLElement, type: 'click' }),
    onKeyDown: (e: KeyboardEvent) => {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      e.preventDefault();
      press({ el: e.currentTarget as HTMLElement, type: 'keydown', key: e.key });
    },
  });
  const has = (): boolean => {
    const a = bound();
    return a ? a.hasSelectedItems : chosen() !== undefined;
  };
  const text = (): string => {
    const a = bound();
    return a ? a.valueAsString : (chosen()?.label ?? '');
  };
  return (
    <div {...(bound()?.getRootProps() ?? { 'data-state': pending() ? 'open' : 'closed' })} data-kit="select" data-part={part()}>
      <button
        {...(bound()?.getTriggerProps() ?? plainTrigger())}
        aria-label={props['aria-label']}
        aria-labelledby={props['aria-labelledby']}
        aria-describedby={props['aria-describedby'] ?? field()?.describedBy}
        data-kit="select-trigger"
        // v1's name for a list's button in dialogs and Settings (ui/select.js .kit-sel), beside the kit's own
        data-part={`${sub(part(), 'trigger')} form.select`}
        data-size={props.size ?? 'md'}
        data-mono={flag(props.mono)}
        data-placeholder={flag(!has())}
      >
        <span data-kit="select-value" data-text="control">{has() ? text() : (props.placeholder ?? '')}</span>
        <Icon name="chevron" size="sm" />
      </button>
      <select {...(bound()?.getHiddenSelectProps() ?? { name: props.name, value: props.value ?? '', disabled: props.disabled === true, 'aria-hidden': true, tabIndex: -1, hidden: true })}>
        <For each={props.items}>{(item) => <option value={item.value}>{item.label}</option>}</For>
      </select>
      <Show when={machine.ready()}>
        {(m) => {
          const M = m().SelectMachine;
          return <M base={props} id={id} invalid={invalid()} triggerId={triggerId()} part={part()} bind={(a) => setApi(() => a)} pending={untrack(pending)} />;
        }}
      </Show>
    </div>
  );
}
