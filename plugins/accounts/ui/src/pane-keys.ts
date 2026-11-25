// The settings Settings → Agents has a row for (SettingsPaneDef.keys): Settings' search opens them here and lands on
// the row (All harnesses and every harness's Advanced have Lean runs, the subagent model and the update check). Not a
// harness's Defaults (runtime.launch) or its stop-at (budget.limitHoldBy): those rows exist only with a harness picked,
// and the search can't know which, so they open on All settings, filtered. Plain, so the search's test reads it.
export const PANE_KEYS = {
  agents: ['runtime.leanRuns', 'runtime.subagentModel', 'accounts.updateCheck'],
} as const satisfies Record<string, readonly string[]>;
