// The settings Settings → Agents has a row for (SettingsPaneDef.keys): Settings' search opens them here and lands on
// the row (a harness's Defaults write runtime.launch, its Limits budget.limitHoldBy; Advanced has Lean runs, the
// subagent model and the update check). Plain, so the search's test reads it without the pane.
export const PANE_KEYS = {
  agents: ['runtime.leanRuns', 'runtime.subagentModel', 'accounts.updateCheck', 'budget.limitHoldBy', 'runtime.launch'],
} as const satisfies Record<string, readonly string[]>;
