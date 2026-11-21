// The goal's view kinds, apart from ./actions.ts so the plugin's index needs only these (ADR-0011).
export const VIEW = {
  /** overlay: Set goal… / Edit goal…; params { task } */
  edit: 'goal.edit',
  /** overlay: each judge's vote and the rounds before; params { task } */
  checks: 'goal.checks',
  /** overlay: one check, read like a task (v1 goal-run.js); params RunHint */
  run: 'goal.run',
} as const;
