// A list drawn by key (B31, B32, the flicker gate qa/flicker.mjs): each item is made once for its key and reads its
// latest value through an accessor, so a list handed in again as new objects (a row view made on every change of
// the store, a menu's entries made on every render) updates its items in place instead of taking every one down and
// putting it back (Solid's <For> keys by the object: a new object is a new element, its hover, focus and entry
// animation lost). Order and membership follow the keys.

import { For, createMemo, type JSX } from 'solid-js';

const same = (a: readonly string[], b: readonly string[]): boolean => a.length === b.length && a.every((v, i) => v === b[i]);

export function ForKeyed<T>(props: {
  readonly each: readonly T[];
  /** what makes an item the same element: its id by default */
  readonly by?: ((item: T) => string) | undefined;
  readonly children: (item: () => T, key: string) => JSX.Element;
}): JSX.Element {
  const keyOf = (x: T): string => (props.by ? props.by(x) : String((x as { readonly id?: unknown }).id));
  const keys = createMemo(() => props.each.map(keyOf), [], { equals: same });
  return (
    <For each={keys()}>
      {(k) => {
        // the latest item for this key; the last one seen while the list drops it (the key leaves in the same pass)
        let last = props.each.find((x) => keyOf(x) === k) as T;
        const item = createMemo(() => {
          const v = props.each.find((x) => keyOf(x) === k);
          if (v !== undefined) last = v;
          return last;
        });
        return props.children(item, k);
      }}
    </For>
  );
}
