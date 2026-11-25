// The settings Settings → Usage has a row for (SettingsPaneDef.keys): Settings' search opens them here (Machine load's
// rows are wisp.machine's, drawn in Usage's end slot). Plain, so the search's test reads it without the pane.
export const PANE_KEYS = {
  usage: [
    'budget.drainWatch',
    'budget.limitHold',
    'budget.liveLimits',
    'budget.silentStop',
    'budget.spareCard',
    'budget.windowNotice',
    'runtime.limitWait',
    'runtime.subagentModel',
    'machine.memoryFloor',
    'machine.cpuCeiling',
    'machine.maxRuns',
  ],
} as const satisfies Record<string, readonly string[]>;
