# ADR-0123: Settings → Agents is a list and the harness picked, each harness has its own settings, and Settings' search lands on the row

- Status: accepted
- Date: 2026-11-25
- Source: owner, settings audit r1 (approved items `user-settings-per-harness`, `settings-search-wrong-pane`,
  `agents-pane-unsearchable`): "be creative: if a different design gives a smoother, faster, clearer experience, do
  it, as long as every existing feature stays reachable."
- With ADR-0003 (everything is a plugin), ADR-0066 (harness detection on a topic), ADR-0091 (Ask near the limit),
  ADR-0115 (the harness asks before lowering a guard).

## Context

Settings → Agents was one flat box: every harness's row, its accounts, the changed-program notice, git and GitHub, Lean
Claude Code runs, the changed-program ask and Re-check, in one column. There was no room for a harness's own settings:
no default model or level for it, no limit of its own, nowhere to see the program wisp pinned, and a harness whose
plugin was off simply vanished (`harnessState` had no Disabled).

Settings' search opened a setting on the one pane of the plugin that declares it, whatever pane has its row:
`channels.project` opened Phone (its row is in Notifications), `worktrees.projectAsk` Worktrees (Privacy has it),
`worktrees.worktree` Worktrees (General has it); and it opened the pane at its top, the row nowhere marked. Agents had
no words to be found by ("api key", "sign in" found nothing), no harness was a result, and `accounts.updateCheck` had no
row anywhere.

## Decision

1. **Agents is a list and the one picked** (the kit's `SettingsSplit`): a filter, **All harnesses** (Update all,
   Re-check everything, git and GitHub, the rows every harness shares), then every harness with its mark, a state dot
   (ready, signed out, missing, checking, disabled) and a ring of its fullest plan window (`budget.view`; none while no
   plan reported). Up, Down, Home and End walk it, Enter and Right go into the detail. Narrow (under 640 px of the
   pane), the list pushes the detail, with Back. **The pick is navigation**: the Settings params' `filter` is
   `harness:<id>`, so a reload, Back and the search land on it.
2. **The detail has five sections**, loaded when a harness is picked: *Account & auth* (its state, Sign in / out, the
   steps, its accounts with Add, Rename, Remove and Move, its keys), *Defaults* (its model and level, written into
   `runtime.launch`'s `models[id]` / `labels[id]` / `efforts[id]` through typed helpers, never as JSON; a saved model it
   no longer lists warns, with Reset), *Limits & budget* (its own Ask near the limit, and the usage rows every harness
   shares, with Open Usage), *Binary* (the pinned program as found and really, its version and Update, Re-detect this
   one only through `runtime.agents { fresh: <id> }`, the changed-program notice and Trust), *Advanced* (its plugin's
   switch through `plugins.setEnabled`, Lean runs and the subagent model for the harness whose UI declares the
   `subagentModel` variant, the changed-program ask, the update check).
3. **A harness whose plugin is off stays listed, Disabled.** A harness plugin's UI stays registered while its server
   half is off, so the pane knows its name and mark; `plugins.list` says which are off (a harness's plugin is
   `wisp.<id>`, else one whose id ends in `.<id>`).
4. **A new per-harness persisted setting, `budget.limitHoldBy`** (wisp.budget's, JSON `{ <harness>: off | 80 | 90 |
   95 }`): each entry is that harness's Ask near the limit, over everyone's `budget.limitHold`; a harness it doesn't
   name keeps everyone's (`ask::line_for`). It is a guard (ADR-0115): a write from the harness that makes any harness's
   line later than it is, or off, or drops an entry whose line was sooner than everyone's, waits on the user's Allow.
5. **The harness row carries its pin** (`accounts.harness.pin`: path, real path, version; migration 4), so Binary shows
   it.
6. **A pane says which settings it has a row for** (`SettingsPaneDef.keys`, additive). Settings' search opens a setting
   on the pane that lists it; else on its plugin's one pane, only when that pane lists nothing (a plugin of yours that
   never said); else All settings, filtered to it. It sends `focus: <key>` in the params, and the kit's `SettingsRow`
   with that `settingKey` scrolls into view, flashes once (none with reduced motion) and focuses its control. Each
   built-in pane's list is a plain `src/pane-keys.ts`, so a test checks every declared setting opens where its row is,
   or on All settings.
7. **Agents is found by its words** (`accounts.pane.keywords`: sign in, api key, install, update, …) and each harness is
   a search result of its own ("Codex — Agents"), opening Agents on it.

## Consequences

- Every row the old pane had is still there: the harness's row, accounts, steps, keys, Install and the guide in Account &
  auth; Update and the changed notice in Binary; Lean runs and the ask in Advanced and in All harnesses; git, GitHub,
  Update all and Re-check in All harnesses.
- Places it reaches (ADR-0040): `budget.limitHoldBy` is a setting like any other (`wisp settings set`, the MCP's
  `settings_set`, guarded as above); Re-detect is `runtime.agents` with `fresh`, already a CLI and MCP read.
- A plugin's pane that doesn't declare `keys` keeps today's routing for its own settings.
- The pane reads `budget.view` for each present harness when it opens and when the plan's rows change: one read per
  harness, never polled.
