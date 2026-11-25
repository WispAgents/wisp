// The settings Settings → Cloud has a row for (SettingsPaneDef.keys): none, so Settings' search opens wisp Cloud's own
// settings on All settings, filtered to them, rather than on a pane without their row.
export const PANE_KEYS = {
  cloud: [],
} as const satisfies Record<string, readonly string[]>;
