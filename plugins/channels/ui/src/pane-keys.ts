// The settings Settings → Phone has a row for (SettingsPaneDef.keys): none. wisp.channels' two settings are rows in
// Notifications, which lists them, so Settings' search opens them there.
export const PANE_KEYS = {
  phone: [],
} as const satisfies Record<string, readonly string[]>;
