// The overlays' phone geometry and steps (ADR-0065 build step 5, design/mobile.md §A.4, §A.7): where the iOS context
// menu puts its preview and its menu, where an M3 plain tooltip sits, which detent a dragged iOS sheet goes to, and the
// rows a long-press menu draws. Pure: no DOM; controls/phone.tsx draws them.
import type { MenuEntry, MenuItem, MenuSubmenu } from './menu-entries.ts';

export interface Box {
  readonly top: number;
  readonly left: number;
  readonly width: number;
  readonly height: number;
}

export interface Viewport {
  readonly width: number;
  readonly height: number;
  /** what the system draws over the top and bottom (the status bar, the home indicator) */
  readonly safeTop: number;
  readonly safeBottom: number;
}

export interface ContextPlace {
  /** the preview's top (it keeps its own left and width) */
  readonly previewTop: number;
  readonly menuTop: number;
  /** the menu's left: under the preview's leading edge, or its trailing edge when the press was on the far half */
  readonly menuLeft: number;
  /** the menu opens below the preview (else above it) */
  readonly below: boolean;
}

/** The margin an iOS context menu keeps from the screen's edges (UIKit: 8 pt beyond the safe area). */
export const CONTEXT_MARGIN = 8;

/**
 * UIContextMenuInteraction's layout: the preview stays where the row was, the menu `gap` below it; when that runs off
 * the bottom the menu goes above; when neither fits, the preview slides up (never off the top) so the menu fits below.
 */
export function contextPlace(row: Box, vp: Viewport, menu: { readonly width: number; readonly height: number }, gap: number, pressX: number): ContextPlace {
  const top = vp.safeTop + CONTEXT_MARGIN;
  const bottom = vp.height - vp.safeBottom - CONTEXT_MARGIN;
  const trailing = pressX > row.left + row.width / 2;
  const rawLeft = trailing ? row.left + row.width - menu.width : row.left;
  const menuLeft = Math.max(CONTEXT_MARGIN, Math.min(rawLeft, vp.width - CONTEXT_MARGIN - menu.width));
  const rowTop = Math.max(top, row.top);
  if (rowTop + row.height + gap + menu.height <= bottom) return { previewTop: rowTop, menuTop: rowTop + row.height + gap, menuLeft, below: true };
  if (rowTop - gap - menu.height >= top) return { previewTop: rowTop, menuTop: rowTop - gap - menu.height, menuLeft, below: false };
  // slide the preview up until the menu fits under it (a tall menu: from the top)
  const previewTop = Math.max(top, bottom - menu.height - gap - row.height);
  return { previewTop, menuTop: previewTop + row.height + gap, menuLeft, below: true };
}

/** An M3 plain tooltip: centred over its anchor, `gap` above it (below when there's no room), inside the screen. */
export function tipPlace(anchor: Box, tip: { readonly width: number; readonly height: number }, vp: Viewport, gap: number): { readonly top: number; readonly left: number } {
  const above = anchor.top - gap - tip.height;
  const top = above >= vp.safeTop ? above : anchor.top + anchor.height + gap;
  const left = Math.max(CONTEXT_MARGIN, Math.min(anchor.left + anchor.width / 2 - tip.width / 2, vp.width - CONTEXT_MARGIN - tip.width));
  return { top, left };
}

/** How long an M3 plain tooltip opened by a long-press stays (M3 Tooltips › Behavior: 1.5 s after the touch ends). */
export const TIP_MS = 1500;

export type Detent = 'medium' | 'large';

/** The detents an iOS sheet offers, in order: both unless the caller names its own (an empty list is both). */
export function detentsOf(given: readonly Detent[] | undefined): readonly Detent[] {
  const list = (given ?? []).filter((d, i, all) => all.indexOf(d) === i);
  return list.length ? [...list].sort((a, b) => (a === b ? 0 : a === 'medium' ? -1 : 1)) : ['medium', 'large'];
}

/** Where a drag leaves an iOS sheet: down from large steps to medium (when it has one), down from the smallest closes;
 * up grows it to large. */
export function detentAfter(detents: readonly Detent[], at: Detent, dir: 'down' | 'up'): Detent | 'close' {
  if (dir === 'up') return detents.includes('large') ? 'large' : at;
  if (at === 'large' && detents.includes('medium')) return 'medium';
  return 'close';
}

/** A long-press menu's rows (the Android sheet, the iOS context menu): groups flattened under their label. */
export type PhoneRow =
  | { readonly kind: 'item'; readonly item: MenuItem }
  | { readonly kind: 'sub'; readonly entry: MenuSubmenu }
  | { readonly kind: 'label'; readonly label: string }
  | { readonly kind: 'separator' };

/** The value of the *Select* row the Android sheet puts first (design/mobile.md §A.4). */
export const SELECT_VALUE = 'menu:select';

export function phoneRows(entries: readonly MenuEntry[], select?: string): PhoneRow[] {
  const rows: PhoneRow[] = [];
  if (select !== undefined) rows.push({ kind: 'item', item: { value: SELECT_VALUE, label: select, icon: 'check' } }, { kind: 'separator' });
  for (const e of entries) {
    if (e.kind === 'separator') {
      // never two in a row, never one first
      if (rows.length && rows[rows.length - 1]?.kind !== 'separator') rows.push({ kind: 'separator' });
    } else if (e.kind === 'group') {
      if (!e.items.length) continue;
      rows.push({ kind: 'label', label: e.label });
      for (const item of e.items) rows.push({ kind: 'item', item });
    } else if (e.kind === 'submenu') rows.push({ kind: 'sub', entry: e });
    else rows.push({ kind: 'item', item: e });
  }
  while (rows[rows.length - 1]?.kind === 'separator') rows.pop();
  return rows;
}

/** The rows' height before they're drawn (the context menu's place is decided as it opens): rows and separators. */
export function rowsHeight(rows: readonly PhoneRow[], rowH: number, sepH: number): number {
  return rows.reduce((h, r) => h + (r.kind === 'separator' ? sepH : rowH), 0);
}
