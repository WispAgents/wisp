// Settings in the phone apps (ADR-0065 step 14): which panes they offer and how the list is sectioned. Its own module,
// imported by the lazy frame only, so the browser's first download (which has nav.ts for parseParams) never carries it.
// Pure, for tests.
import type { SettingsNavGroup } from '@wisp/ui';
import { groupPanes, rank, type PaneEntry } from './nav.ts';

/** The phone apps' list (design/mobile.md §A.2 Settings: M3 settings with section headers, iOS's inset grouped
 * table): the panes in sections by their group, in the contract's order, each named; within one, v1's order. Pure. */
export function phoneGroups(entries: readonly PaneEntry[], title: (key: string) => string, groupLabel: (id: string) => string): SettingsNavGroup[] {
  const flat = groupPanes(entries, title)[0]?.items ?? [];
  const groupOf = new Map(entries.map((e) => [e.pane.id, e.pane.group]));
  const ids = [...new Set(flat.map((i) => groupOf.get(i.id) ?? ''))].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
  return ids.map((g) => ({ id: g, label: groupLabel(g), items: flat.filter((i) => (groupOf.get(i.id) ?? '') === g) }));
}

/** Panes a phone app leaves out: keyboard shortcuts (v1 F-set-6: the Android app hid its shortcuts sheet). */
export const PHONE_HIDDEN: ReadonlySet<string> = new Set(['keys']);

/** The panes this frame offers: all of them, less what a phone app hides. */
export function offeredPanes(panes: readonly PaneEntry[], phone: boolean): readonly PaneEntry[] {
  return phone ? panes.filter((p) => !PHONE_HIDDEN.has(p.pane.id)) : panes;
}
