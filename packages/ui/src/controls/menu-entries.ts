// Menu entries and menuSections, plain (no component): the shape both Menu and ContextMenu take, importable from
// `@wisp/ui/pure` so a module that builds a menu (the shell's task menu) and its test skip the kit.
import type { PartName } from '../core/props.ts';
import type { IconName } from '../content/icons.ts';
import { BREAKPOINTS } from '@wisp/tokens';

/** The screens where the browser's menus open as a sheet: touch-first, or narrower than the phone breakpoint. Here (a
 *  plain module) so a lazy picker can ask too (the shell's All goals…, owner-list #63). */
export const SHEET_SCREEN = `(pointer: coarse), (max-width: ${BREAKPOINTS.phone}px)`;

export interface MenuItem {
  readonly kind?: 'item';
  readonly value: string;
  /** its own part name (a v1 part such as `topbar.settings`), instead of the menu's `<part>.item` */
  readonly part?: PartName;
  readonly label: string;
  readonly icon?: IconName;
  /** A second line under the label. */
  readonly hint?: string;
  /** The shortcut, shown at the end (hidden on touch). */
  readonly keys?: string;
  /** Red on hover: a destructive action. */
  readonly danger?: boolean;
  readonly disabled?: boolean;
  /** A toggle that is on shows a check at the end. */
  readonly checked?: boolean;
}

export interface MenuSeparator {
  readonly kind: 'separator';
}

export interface MenuGroup {
  readonly kind: 'group';
  readonly id: string;
  readonly label: string;
  readonly items: readonly MenuItem[];
}

/** A row that drills into its own items (one level: no submenu in a submenu). */
export interface MenuSubmenu {
  readonly kind: 'submenu';
  /** names the row (its `data-value`); never passed to `onSelect` */
  readonly value: string;
  readonly label: string;
  readonly icon?: IconName;
  readonly part?: PartName;
  readonly items: readonly (MenuItem | MenuSeparator | MenuGroup)[];
  /** A box over its rows that narrows them as you type (a menu that grows with the formula: Goal…, Recipes), on each
   *  row's label and second line; Enter picks the first match. Additive (2026-11-20, owner-list #63). */
  readonly filter?: MenuFilter;
}

export interface MenuFilter {
  /** the box's name, for a screen reader (Filter goals) */
  readonly label: string;
  readonly placeholder?: string;
  /** the row shown when nothing matches (No goal matches) */
  readonly empty: string;
  /** what typing searches, when the rows show only some of them (the first few, then All goals…); else the rows */
  readonly all?: readonly MenuItem[];
  /** rows that stay under the matches whatever is typed (New goal…) */
  readonly keep?: readonly string[];
}

export type MenuEntry = MenuItem | MenuSeparator | MenuGroup | MenuSubmenu;

/** A long menu in sections (v1 #786 sectionsHtml / subHtml): a line between groups, empty groups
 *  dropped, and a submenu of one item left inline (none of zero), so no drill-in holds a single row. */
export function menuSections(groups: readonly (readonly (MenuEntry | false | null | undefined)[])[]): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const g of groups) {
    const xs = g.flatMap((e): MenuEntry[] => {
      if (!e) return [];
      if (e.kind !== 'submenu') return [e];
      return e.items.length < 2 ? [...e.items] : [e];
    });
    if (!xs.length) continue;
    if (out.length) out.push({ kind: 'separator' });
    out.push(...xs);
  }
  return out;
}

/** The entries as a menu draws them: never a separator first, last or twice in a row (a menu whose only item followed
 *  a separator showed a line over it: Machines' ⋯ with Sign out of the grid alone). */
export function tidySeparators<E extends object>(items: readonly E[]): E[] {
  const sep = (e: E | undefined): boolean => (e as { readonly kind?: string } | undefined)?.kind === 'separator';
  const out: E[] = [];
  for (const e of items) {
    if (sep(e) && (out.length === 0 || sep(out[out.length - 1]))) continue;
    out.push(e);
  }
  while (out.length && sep(out[out.length - 1])) out.pop();
  return out;
}
