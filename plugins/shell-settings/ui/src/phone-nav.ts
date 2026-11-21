// Settings' sections (ADR-0065 step 14 on phones; the browser and desktop too since TRY-28) and which panes a phone app
// offers. Its own module, imported by the lazy frame only, so the browser's first download (which has nav.ts for
// parseParams) never carries it. Pure, for tests.
import type { SettingsNavGroup } from '@wisp/ui';
import { groupPanes, rank, type PaneEntry } from './nav.ts';

/** Settings' list: the panes in sections by their group, in the contract's order, each named; within one, v1's order.
 * Phones draw it as M3 settings with section headers / iOS's inset grouped table (design/mobile.md §A.2); the browser
 * and desktop as headed groups in the side list (TRY-28, allowed.md §7 `settings.sections`: v1 had one flat list,
 * which at 22 panes had to be read top to bottom). Pure. */
export function sectionedPanes(entries: readonly PaneEntry[], title: (key: string) => string, groupLabel: (id: string) => string): SettingsNavGroup[] {
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

/** Settings' search (TRY-28): the sections keeping only the panes whose name, section or description has every word
 * typed (any order, any case); a section left empty goes. An empty query keeps them all. Pure. */
export function filterSections(groups: readonly SettingsNavGroup[], query: string, about: (paneId: string) => string): SettingsNavGroup[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...groups];
  return groups
    .map((g) => ({
      ...g,
      items: g.items.filter((i) => {
        const text = `${i.label} ${g.label ?? ''} ${i.id} ${about(i.id)}`.toLowerCase();
        return words.every((w) => text.includes(w));
      }),
    }))
    .filter((g) => g.items.length > 0);
}
