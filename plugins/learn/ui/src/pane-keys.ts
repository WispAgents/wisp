// The settings Settings → Memory has a row for (SettingsPaneDef.keys). Plain, so the search's test reads it.
export const PANE_KEYS = {
  memory: ['learn.enabled'],
} as const satisfies Record<string, readonly string[]>;
