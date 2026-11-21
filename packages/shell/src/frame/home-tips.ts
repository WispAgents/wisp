// UX 2026-11-18 desktop #8: Home shows one tip, from one list: the setup cards (connect GitHub, the Grid) first, then
// the plain tips. More tips moves one on and comes round again. DOM-free.
export type TipPick = { readonly kind: 'setup'; readonly i: number } | { readonly kind: 'tip'; readonly id: string };

/** The tip at position `at` of `setup` setup cards followed by `tips`; undefined when there are none. */
export function tipPick(at: number, setup: number, tips: readonly string[]): TipPick | undefined {
  const n = setup + tips.length;
  if (n === 0) return undefined;
  const i = ((at % n) + n) % n;
  if (i < setup) return { kind: 'setup', i };
  const id = tips[i - setup];
  return id === undefined ? undefined : { kind: 'tip', id };
}
