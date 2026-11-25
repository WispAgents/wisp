// The settings Settings → Privacy has a row for (SettingsPaneDef.keys): Settings' search opens them here. Plain, so the
// search's test reads it without the pane.
export const PANE_KEYS = {
  privacy: ['git.maskEnvFiles', 'worktrees.projectAsk'],
} as const satisfies Record<string, readonly string[]>;
