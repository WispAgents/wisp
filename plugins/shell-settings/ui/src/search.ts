// Settings' search over the settings themselves (audit A F11): every declared setting (settings.list) by its label,
// its description or its key, and where each opens: the pane of the plugin that declares it when that plugin has one
// pane, else All settings filtered to it. Pure, for tests.

/** A declared setting as the search reads it (its title split as All settings shows it). */
export interface FoundSetting {
  readonly key: string;
  readonly owner: string;
  readonly label: string;
  readonly help: string;
}

/** The settings with every word typed in their label, description or key (any case; a key's dots and case ignored). */
export function foundSettings(all: readonly FoundSetting[], query: string): FoundSetting[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];
  return all.filter((s) => {
    const text = `${s.label} ${s.help} ${s.key} ${s.key.replace(/[._]/g, '')}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
}

/** Where a found setting opens: its plugin's one pane, else All settings on it. */
export function settingTarget(s: Pick<FoundSetting, 'key' | 'owner'>, panes: readonly { readonly id: string; readonly plugin: string }[]): { pane: string; filter?: string } {
  const own = panes.filter((p) => p.plugin === s.owner);
  if (own.length === 1 && own[0]) return { pane: own[0].id };
  return { pane: 'all', filter: s.key };
}
