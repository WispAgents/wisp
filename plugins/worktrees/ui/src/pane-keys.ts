// The settings Settings → Worktrees has a row for (SettingsPaneDef.keys): Settings' search opens them here (the worktree
// default is General's, the project ask Privacy's). Plain, so the search's test reads it without the pane.
export const PANE_KEYS = {
  worktrees: ['worktrees.cleanup', 'worktrees.max', 'worktrees.onDone'],
} as const satisfies Record<string, readonly string[]>;
