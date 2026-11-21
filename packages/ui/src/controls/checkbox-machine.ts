// Checkbox's machine half (Zag's checkbox: focus-visible, hover and the hidden input's state on every part). Loaded
// apart from the first page (./lazy-part.ts) and bound to the box Switch.tsx draws at once with a plain input.
import * as checkbox from '@zag-js/checkbox';
import { normalizeProps, useMachine } from '@zag-js/solid';
import { createMemo, type Accessor } from 'solid-js';
import type { CheckboxProps } from './Switch.tsx';

export type CheckboxApi = checkbox.Api;

export function useCheckbox(props: CheckboxProps, id: string): Accessor<checkbox.Api> {
  const service = useMachine(checkbox.machine, () => ({
    id,
    checked: props.checked,
    // An indeterminate box becomes checked when clicked; callers only ever see a boolean.
    onCheckedChange: (d: checkbox.CheckedChangeDetails) => props.onChange(d.checked === true),
    ...(props.disabled === undefined ? {} : { disabled: props.disabled }),
    ...(props.invalid === undefined ? {} : { invalid: props.invalid }),
    ...(props.name === undefined ? {} : { name: props.name }),
    ...(props.value === undefined ? {} : { value: props.value }),
  }));
  return createMemo(() => checkbox.connect(service, normalizeProps));
}
