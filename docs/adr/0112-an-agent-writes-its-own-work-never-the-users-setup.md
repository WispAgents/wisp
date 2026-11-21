# ADR-0112: an agent writes its own work, never the user's setup, and never raises a mode

- Status: accepted
- Date: 2026-11-20
- Source: owner's delegate, review r194 round 3 (row 61): "Agents may not write the user's setup … unless the user
  explicitly granted it; narrow [the agent's set] to what an agent's work needs."
- Changes: ADR-0008's `agent:session` set (and every set built from it: `agent:session:<mode>`, `agent:capped*`,
  `crew:<name>`); ADR-0040/ADR-0077's agent-side `runtime_projectAdd`; v1's in-task `wisp_crew_add`/`wisp_crew_edit`/
  `wisp_crew_remove` and `wisp_connector_add` (they stay for the user's own harness through `wisp mcp`).

## Context

A task's agent held 161 write commands. A probe from an agent's token showed what that meant: `runtime.setMode` raised its
own plan task to Full auto (so the children it started were allowed Full auto); `connectors.add` stored `sh -c id`, enabled,
and wisp tested it at once, and every later task got it; `runtime.profile.set` and `crew.create` (at Full auto) rewrote the
user's agents and crew; `runtime.card` wrote any plugin's `ext`.

## Decision

1. **An agent never raises a mode.** `runtime.setMode` from an agent's token may lower or keep a task's mode, never raise
   it (its own task included): `wisp_runtime_core::child::set_mode`. An agent's follow-up can't lift its task either.
   Only the user raises a mode (the app, the CLI, a channel the user allows).
2. **An agent writes its own work, not the user's setup.** `agent:session` keeps its allow list and denies (with the
   reason the agent reads) every write that changes the user's setup or skips the user: connectors (add, agent.add,
   update, remove, import, test, sign-in/out), crew writes and sharing, agent profiles, adding projects, importing or
   adopting sessions, harness and sandbox sign-ins, `setExt`/`turn.setExt`, approving what it was blocked from, overage,
   Try it two ways, machines (add, remove, update, connect, disconnect, let in, take in, follow, stop, recheck), a
   sandbox's environment, publishing apps or widening an app's grant, and the time zone and wait-at-limit settings. It
   never held settings, secrets, plugin installs, triggers, automations or `resources.apply`.
3. **What an agent may write** (`serve_mcp_tools::an_agents_capability_set_writes_only_its_own_work` holds this list
   exactly; a new write command is a decision here): `connectors.call`; `crew.agent.kit` (a proposal the user picks from),
   `crew.ask`, `crew.assign`, `crew.checkin`, `crew.export`, `crew.reply`, `crew.wake`, `crew.wakeMentions`; `git.checkout`,
   `git.clone`, `git.commit`, `git.restoreTurn`, `git.revert`, `git.stage`; `goal.check`, `goal.drop`, `goal.pause`,
   `goal.resume`, `goal.set`; `history.resumeRequest`; `later.add`, `later.agent.schedule`, `later.cancel`,
   `later.cancelPlan`, `later.done`, `later.edit`, `later.editPlan`, `later.park`, `later.plan`, `later.tick`;
   `machines.bring`, `machines.call`, `machines.carry`, `machines.dropCopies`, `machines.route`, `machines.send`,
   `machines.uncarry`; `mcp.rpc`; `runtime.agent.tag`, `runtime.agent.task`, `runtime.agent.watch`, `runtime.answer`,
   `runtime.appState`, `runtime.approval` (asking the user), `runtime.archive`, `runtime.ask`, `runtime.btw`,
   `runtime.btw.drop`, `runtime.card` (no `ext`), `runtime.compact`, `runtime.create` (held to its parent, ADR-0111),
   `runtime.delegate`, `runtime.delete`, `runtime.exec`, `runtime.followUp`, `runtime.forLater`, `runtime.fork`,
   `runtime.held` and `runtime.release` (an agent's is never a person's yes), `runtime.markRead`, `runtime.note`,
   `runtime.pin`, `runtime.probe`, `runtime.projectRun`, `runtime.projectTest` (the project's own declared commands),
   `runtime.queue.*`, `runtime.quick`, `runtime.quickCancel`, `runtime.rename`, `runtime.rerun`, `runtime.restoreSession`,
   `runtime.reveal`, `runtime.setAgent`, `runtime.setLean`, `runtime.setMode` (never up), `runtime.setState`,
   `runtime.settle`, `runtime.stop`, `runtime.terminal`, `runtime.unask`; `sandbox.bring`, `sandbox.drop`,
   `sandbox.launch`, `sandbox.own`, `sandbox.place`, `sandbox.seed`; `ship.agent.pr`; `stuff.addStarter`,
   `stuff.agent.save`, `stuff.launch`, `stuff.receive`, `stuff.remove`, `stuff.removeUpload`, `stuff.resetData`,
   `stuff.restore`, `stuff.reveal`, `stuff.save`, `stuff.stop`, `stuff.update`; `threads.fresh`, `threads.notesUpdate`.
   The kernel's agent scope still holds each of these to the agent's own task where they name one.
4. **No command an agent wrote runs.** Connectors and MCP servers (whose `command` runs on the host for every task), the
   `tools`, `harnesses` and `projects` kinds, plugin installs and the settings that hold a command (editor, terminal) are
   all the user's; an agent's formula can't declare them either (ADR-0111).
5. **A card an agent makes carries no `ext`** (plugins write their own fields).
6. A deny the kernel already had for a capped task keeps its v1 words when `agent:session` denies the same permission:
   the first reason for a permission stands (`caps::with`).

## Consequences

- In-task `wisp_crew_add`/`wisp_crew_edit`/`wisp_crew_remove`, `wisp_connector_add` and `runtime_projectAdd` are refused
  (or not listed, ADR-0107) for an agent; the user's own harness keeps them through `wisp mcp` (user caller). A user who
  wants an agent to set something up asks it to say what, and does it (or runs it from `wisp mcp`). A per-user grant
  ("agents may change my setup") would be a new ADR.
- `wisp_connector_add` from a task no longer reaches the user's card; it says the user adds connectors.
- Not checkable by `tools/check`: the exact list is a test (above), and each rule has its own (below).

## Tests

- `crates/wisp/tests/serve_mcp_tools.rs`: `an_agent_never_raises_a_tasks_mode`, `an_agent_never_adds_or_changes_a_connector`,
  `an_agents_capability_set_writes_only_its_own_work`, `an_agents_card_carries_no_ext`,
  `the_users_harness_adds_a_project_through_its_mcp_tool_and_the_server_names_it`,
  `wisp_crew_edit_changes_only_the_fields_given_and_remembers_lines`, `wisp_crew_remove_removes_a_member`,
  `wisp_connector_add_is_the_users_and_takes_key_names`
- `crates/wisp-kernel/src/caps.rs`: `capped_uses_v1_words` (the first reason stands)
