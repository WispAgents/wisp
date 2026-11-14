// Select's machine half (Zag's select, floating-ui): typeahead, arrow keys, Home/End, its options in a menu-like list.
// Loaded apart from the first page (./lazy-part.ts) and bound to the button Select.tsx draws at once.
import * as select from '@zag-js/select';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, For, onMount, Show, type Accessor, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { sub, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { SelectOption, SelectProps } from './Select.tsx';
import { bound, type Replay } from './lazy-part.ts';

export type SelectApi = select.Api;

export const optionCollection = (items: readonly SelectOption[]) =>
  select.collection<SelectOption>({
    items: [...items],
    itemToValue: (i) => i.value,
    itemToString: (i) => i.label,
    isItemDisabled: (i) => Boolean(i.disabled),
  });

export interface SelectMachineProps {
  readonly base: SelectProps;
  /** the machine's id (the plain button already carries its trigger's id) */
  readonly id: string;
  readonly invalid: boolean;
  /** the trigger's id: the Field's (its label points at the button), or the caller's */
  readonly triggerId: string | undefined;
  readonly part: PartName;
  readonly bind: (api: Accessor<select.Api>) => void;
  /** pressed before the machine was there: that press again, now */
  readonly pending: Replay | undefined;
}

export function SelectMachine(props: SelectMachineProps): JSX.Element {
  const service = useMachine(select.machine, () => ({
    id: props.id,
    collection: optionCollection(props.base.items),
    value: props.base.value === undefined ? [] : [props.base.value],
    onValueChange: (d: select.ValueChangeDetails<SelectOption>) => {
      const next = d.value[0];
      if (next !== undefined) props.base.onChange(next);
    },
    positioning: { placement: 'bottom-start' as const, gutter: 4, sameWidth: true },
    invalid: props.invalid,
    ...(props.base.disabled === undefined ? {} : { disabled: props.base.disabled }),
    ...(props.base.name === undefined ? {} : { name: props.base.name }),
    ...(props.triggerId ? { ids: { trigger: props.triggerId } } : {}),
  }));
  const api = createMemo(() => select.connect(service, normalizeProps));
  props.bind(api);
  onMount(() => bound(api().getTriggerProps().id, props.pending));
  const part = (): PartName => props.part;
  const items = (): readonly SelectOption[] => props.base.items;
  return (
    <Portal>
      <div {...api().getPositionerProps()} data-kit="positioner">
        <ul {...api().getContentProps()} data-kit="listbox" data-part={sub(part(), 'list')}>
          <For each={items()}>
            {(item, i) => (
              <>
                <Show when={item.group && items()[i() - 1]?.group !== item.group}>
                  <li role="presentation" data-kit="option-group" data-part={sub(part(), 'group')}>
                    {item.group}
                  </li>
                </Show>
                <li {...api().getItemProps({ item })} data-kit="option" data-part={sub(part(), 'option')}>
                  <span {...api().getItemTextProps({ item })} data-kit="option-label">
                    {item.label}
                  </span>
                  <Show when={api().getItemState({ item }).selected}>
                    <Icon name="check" size="sm" tone="accent-ink" />
                  </Show>
                </li>
              </>
            )}
          </For>
        </ul>
      </div>
    </Portal>
  );
}
