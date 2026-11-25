// The settings Settings → Updates has a row for (SettingsPaneDef.keys). Plain, so the search's test reads it.
export const PANE_KEYS = {
  updates: ['update.updateChannel'],
} as const satisfies Record<string, readonly string[]>;
