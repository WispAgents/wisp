// The settings Settings → Automations has a row for (SettingsPaneDef.keys). Plain, so the search's test reads it.
export const PANE_KEYS = {
  automations: ['automations.webhookTriggers'],
} as const satisfies Record<string, readonly string[]>;
