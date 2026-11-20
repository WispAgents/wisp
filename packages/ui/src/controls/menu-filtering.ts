// A submenu's filter, pure (owner-list #63): which rows show for what's typed, and what Enter picks. Apart from
// ./menu-entries.ts (the first page's): only the menus' machines and the shell's lazy pickers read it.
import type { MenuItem, MenuSubmenu } from './menu-entries.ts';

/** The value of the row a filter shows when nothing matches: never chosen. */
export const NO_MATCH = 'menu:no-match';

/** Each word typed is in the row's label or its second line, any case. */
export const matchesQuery = (item: Pick<MenuItem, 'label' | 'hint'>, query: string): boolean => {
  const hay = `${item.label}\n${item.hint ?? ''}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
};

/** A submenu's rows for what's typed in its filter: its own rows while nothing is; else the matches (from `all`, or its
 *  rows), then the rows it keeps after a line; one disabled row says when nothing matches. */
export function filteredItems(s: MenuSubmenu, query: string): MenuSubmenu['items'] {
  const f = s.filter;
  if (!f || !query.trim()) return s.items;
  const keep = new Set(f.keep ?? []);
  const flat = s.items.flatMap((e): MenuItem[] => (e.kind === 'separator' ? [] : e.kind === 'group' ? [...e.items] : [e]));
  const pool = f.all ?? flat;
  const found = pool.filter((i) => !keep.has(i.value) && !i.disabled && matchesQuery(i, query));
  const kept = flat.filter((i) => keep.has(i.value));
  const head: MenuItem[] = found.length > 0 ? found : [{ value: NO_MATCH, label: f.empty, disabled: true }];
  return kept.length > 0 ? [...head, { kind: 'separator' }, ...kept] : head;
}

/** What Enter in the filter picks: the first row it shows that can be chosen. */
export function firstMatch(items: MenuSubmenu['items']): string | undefined {
  for (const e of items) {
    if (e.kind === 'separator') continue;
    const i = e.kind === 'group' ? e.items.find((x) => !x.disabled) : e.disabled ? undefined : e;
    if (i) return i.value;
  }
  return undefined;
}
