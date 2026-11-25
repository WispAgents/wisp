// Settings' search over the settings themselves (audit A F11): every declared setting (settings.list) by its label,
// its description or its key, and where each opens: the pane that has its row (its `keys`), else the pane of the
// plugin that declares it when that plugin has one pane that doesn't say what it shows, else All settings filtered to
// it. Each harness is a result of its own too, opening Settings → Agents on it. Pure, for tests.

import { AUTO_SETTING } from '@wisp/shell/pure';

/** A declared setting as the search reads it (its title split as All settings shows it). */
export interface FoundSetting {
  readonly key: string;
  readonly owner: string;
  readonly label: string;
  readonly help: string;
}

/** A pane as the search routes to it: its plugin, and the settings it has a row for (none said: unknown). */
export interface RoutePane {
  readonly id: string;
  readonly plugin: string;
  readonly keys?: readonly string[] | undefined;
}

/** The settings with every word typed in their label, description or key (any case; a key's dots and case ignored).
 *  Never Auto's switch: All settings has no row for it while Auto is put away (owner, 2026-11-17; setup audit B6), and
 *  Settings → Auto holds it while it's on. */
export function foundSettings(all: readonly FoundSetting[], query: string): FoundSetting[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return all.filter((s) => {
    if (s.key === AUTO_SETTING) return false;
    const text = `${s.label} ${s.help} ${s.key} ${s.key.replace(/[._]/g, '')}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
}

/** Where a found setting opens, and the row to land on (`focus`). The pane that lists it, first (channels.project's
 *  row is in Notifications, not in its plugin's Phone pane); else its plugin's one pane, if that pane lists nothing
 *  (a plugin that never said); else All settings, filtered to it (a setting no pane has a row for: Machine's battery
 *  hold). All settings always opens filtered: it has a row for every setting. */
export function settingTarget(s: Pick<FoundSetting, 'key' | 'owner'>, panes: readonly RoutePane[]): { pane: string; filter?: string; focus?: string } {
  const all = { pane: 'all', filter: s.key };
  const lists = panes.find((p) => p.id !== 'all' && p.keys?.includes(s.key));
  if (lists) return { pane: lists.id, focus: s.key };
  const own = panes.filter((p) => p.plugin === s.owner);
  if (own.length === 1 && own[0] && own[0].id !== 'all' && own[0].keys === undefined) return { pane: own[0].id, focus: s.key };
  return all;
}

/** A harness as the search finds it (the shell's registered harnesses). */
export interface FoundHarness {
  readonly id: string;
  readonly name: string;
}

/** The harnesses whose name (or id) has every word typed: each opens Settings → Agents on it. */
export function foundHarnesses(all: readonly FoundHarness[], query: string): FoundHarness[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return all.filter((h) => words.every((w) => `${h.name} ${h.id}`.toLowerCase().includes(w)));
}

/** Where a harness result opens: Settings → Agents, on it (Agents reads `harness:<id>` as its pick). */
export const harnessTarget = (id: string): { pane: string; filter: string } => ({ pane: 'agents', filter: `harness:${id}` });
