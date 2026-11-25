// The settings the Machine pane has a row for (SettingsPaneDef.keys): none. Its load rows are drawn in Settings → Usage
// (which lists them), and the battery hold has no row: Settings' search opens those on Usage and All settings.
export const PANE_KEYS = {
  machine: [],
} as const satisfies Record<string, readonly string[]>;
