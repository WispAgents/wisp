// Settings' search over the settings themselves (audit A F11): every declared setting (settings.list) by its label,
// its description or its key, and where each opens: the pane of the plugin that declares it when that plugin has one
// pane, else All settings filtered to it. Pure, for tests.

import { AUTO_SETTING } from '@wisp/shell/pure';

/** A declared setting as the search reads it (its title split as All settings shows it). */
export interface FoundSetting {
  readonly key: string;
  readonly owner: string;
  readonly label: string;
  readonly help: string;
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

/** The rows each harness's detail in Settings → Agents holds (harness audit r2), by their title keys: the search finds
 *  "Cursor: Default model" and opens Agents on Cursor. */
export const HARNESS_ROWS = [
  'shell-settings.harness.signIn',
  'shell-settings.harness.program',
  'shell-settings.harness.enabled',
  'shell-settings.harness.model',
  'shell-settings.harness.effort',
  'shell-settings.harness.mode',
  'shell-settings.harness.stop',
] as const;
const AGENTS = 'agents';
const ROW = 'agents:';

/** One search hit per harness and row of its detail (`titles`: the rows' words). */
export function harnessRows(harnesses: readonly { readonly id: string; readonly name: string }[], titles: readonly string[]): FoundSetting[] {
  return harnesses.flatMap((h) => titles.map((title, i) => ({ key: `${ROW}${h.id}:${i}`, owner: AGENTS, label: `${h.name}: ${title}`, help: '' })));
}

/** The harness a declared setting is about, by its key's last part (`budget.autoStopCursor` → cursor), when the
 *  Agents pane has a detail for it. */
export function harnessOfKey(key: string, harnesses: readonly string[]): string | undefined {
  const tail = (key.split('.').pop() ?? '').toLowerCase();
  return harnesses.find((id) => tail !== id && tail.endsWith(id.toLowerCase()));
}

/** Where a found setting opens: a harness's row (or a setting about one harness) on its detail in Agents, else its
 *  plugin's one pane, else All settings on it. */
export function settingTarget(
  s: Pick<FoundSetting, 'key' | 'owner'>,
  panes: readonly { readonly id: string; readonly plugin: string }[],
  harnesses: readonly string[] = [],
): { pane: string; filter?: string } {
  const hasAgents = panes.some((p) => p.id === AGENTS);
  if (hasAgents && s.key.startsWith(ROW)) return { pane: AGENTS, filter: s.key.slice(ROW.length).split(':')[0] ?? '' };
  const about = hasAgents ? harnessOfKey(s.key, harnesses) : undefined;
  if (about) return { pane: AGENTS, filter: about };
  const own = panes.filter((p) => p.plugin === s.owner);
  if (own.length === 1 && own[0]) return { pane: own[0].id };
  return { pane: 'all', filter: s.key };
}
